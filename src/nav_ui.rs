use std::collections::{HashMap, VecDeque};
use std::io;
use std::net::UdpSocket;
use std::ops::DerefMut;
use std::sync::mpsc::Sender;

use conrod::{
    Background,
    Button,
    Color,
    Colorable,
    Frameable,
    Label,
    Labelable,
    Positionable,
    Slider,
    Sizeable,
    TextBox,
    Ui,
    WidgetId,
    Widget,
};
use conrod::color::{rgb, white};
use graphics::Context;
use opengl_graphics::GlGraphics;
use opengl_graphics::glyph_cache::GlyphCache;
use piston::input;
use time;

use imu;
use video_stream::RecordMsg;

enum MissionTime {
    Paused(time::Duration),
    Running(time::Tm, time::Duration),
}

pub struct NavigationUi {
    bg_color: Color,

    mission_time: MissionTime,

    // IMU
    pitch_roll_heading: Option<(f64, f64, f64)>,
    roll: imu::Roll,
    heading: imu::Heading,

    // GPS
    latitude: Option<f64>,
    longitude: Option<f64>,
    speed: Option<f64>,
    altitude: Option<f64>,
    angle: Option<f64>,

    // RPM stuff
    pub l_rpm: f32,
    pub r_rpm: f32,
    pub max_rpm: f32,

    pub motor_speed: f32,

    pub sadl: f32,
    pub last_sadl_time: time::Tm,

    pub blade: f32,

    // Forward camera controls
    pub f_pan: f32,
    pub f_panning: f32,
    pub last_f_pan_time: time::Tm,
    pub f_tilt: f32,
    pub f_tilting: f32,
    pub last_f_tilt_time: time::Tm,

    pub command: String,
    pub command_mode: bool,

    socket: UdpSocket,
    vid0_t: Sender<RecordMsg>,
    vid1_t: Sender<RecordMsg>,
    vid2_t: Sender<RecordMsg>,
    mission_folder: String,

    out_queue: VecDeque<(time::Tm, time::Duration, Vec<u8>, (String, u16))>, // Outbound packet queue
}

impl NavigationUi {
    pub fn new(socket: UdpSocket,
               vid0_t: Sender<RecordMsg>,
               vid1_t: Sender<RecordMsg>,
               vid2_t: Sender<RecordMsg>,
               mission_folder: String) -> NavigationUi {
        NavigationUi {
            bg_color: rgb(0.2, 0.35, 0.45),

            mission_time: MissionTime::Paused(time::Duration::zero()),

            pitch_roll_heading: None,
            roll: imu::Roll::new(),
            heading: imu::Heading::new(),

            latitude: None,
            longitude: None,
            speed: None,
            altitude: None,
            angle: None,

            l_rpm: 0.0,
            r_rpm: 0.0,
            max_rpm: 100.0,

            motor_speed: 1.0,

            sadl: 0.0,
            last_sadl_time: time::now(),

            blade: 0.0,

            f_pan: 90.0,
            f_panning: 0.0,
            last_f_pan_time: time::now(),
            f_tilt: 130.0,
            f_tilting: 0.0,
            last_f_tilt_time: time::now(),

            command: "".to_string(),
            command_mode: false,

            socket: socket,
            vid0_t: vid0_t,
            vid1_t: vid1_t,
            vid2_t: vid2_t,
            mission_folder: mission_folder,

            out_queue: VecDeque::new(),
        }
    }

    pub fn update(&mut self, dt: f64) {
        let dt = dt as f32;

        self.f_pan += self.f_panning*180.0*dt; // 180 degrees per second
        self.f_tilt += self.f_tilting*90.0*dt; // 90 degrees per second

        self.flush_out_queue();
    }

    pub fn draw_ui<'a>(&mut self, c: Context, gl: &mut GlGraphics, ui: &mut Ui<GlyphCache<'a>>) {
        use graphics::*;

        // Draw the background.
        Background::new().color(self.bg_color).draw(ui, gl);

        let time_now = time::now();

        // Local time
        Label::new(format!("{}", time_now.strftime("Local  %x  %X").unwrap()).as_str())
            .xy((-ui.win_w / 2.0) + 100.0, (ui.win_h / 2.0) - 10.0)
            .font_size(16)
            .color(self.bg_color.plain_contrast())
            .set(LOCAL_TIME, ui);

        // UTC time
        Label::new(format!("{}", time_now.to_utc().strftime("%Z  %x  %X").unwrap()).as_str())
            .xy((-ui.win_w / 2.0) + 104.0, (ui.win_h / 2.0) - 30.0)
            .font_size(16)
            .color(self.bg_color.plain_contrast())
            .set(UTC_TIME, ui);

        // Mission time label
        let mission_time =
            match self.mission_time {
                MissionTime::Paused(t) => t,
                MissionTime::Running(start_time, extra_time) =>
                    (time::now() - start_time) + extra_time
            };
        let total_days = mission_time.num_days();
        let total_hours = mission_time.num_hours();
        let total_minutes = mission_time.num_minutes();
        let total_seconds = mission_time.num_seconds();

        let days = total_days;
        let hours = total_hours - total_days*24;
        let minutes = total_minutes - total_hours*60;
        let seconds = total_seconds - total_minutes*60;
        Label::new(format!("Mission Time: {}:{}:{}:{}", days, hours, minutes, seconds).as_str())
            .xy((-ui.win_w / 2.0) + 150.0, (ui.win_h / 2.0) - 70.0)
            .font_size(20)
            .color(self.bg_color.plain_contrast())
            .set(MISSION_TIME_LABEL, ui);

        // Mission start/pause button
        let mission_start_text =
            match self.mission_time {
                MissionTime::Paused(_) => "Start",
                MissionTime::Running(_, _) => "Pause",
            };
        Button::new()
            .dimensions(100.0, 30.0)
            .xy((-ui.win_w / 2.0) + 55.0, (ui.win_h / 2.0) - 100.0)
            .rgb(0.3, 0.8, 0.3)
            .frame(1.0)
            .label(mission_start_text)
            .react(|| {
                match self.mission_time {
                    MissionTime::Paused(current_time) => {
                        self.mission_time = MissionTime::Running(time::now(), current_time);

                        self.vid0_t.send(RecordMsg::Start(format!("mission_data/{}/forward.mkv", self.mission_folder)));
                        self.vid1_t.send(RecordMsg::Start(format!("mission_data/{}/reverse.mkv", self.mission_folder)));
                        self.vid2_t.send(RecordMsg::Start(format!("mission_data/{}/hazard.mkv", self.mission_folder)));
                    },
                    MissionTime::Running(start_time, extra_time) => {
                        self.mission_time = MissionTime::Paused((time::now() - start_time) + extra_time);

                        self.vid0_t.send(RecordMsg::Stop);
                        self.vid1_t.send(RecordMsg::Stop);
                        self.vid2_t.send(RecordMsg::Stop);
                    },
                };
            })
            .set(MISSION_START_BUTTON, ui);

        // Mission reset button
        Button::new()
            .dimensions(100.0, 30.0)
            .xy((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 100.0)
            .rgb(0.3, 0.8, 0.3)
            .frame(1.0)
            .label("Reset")
            .react(|| {
                self.mission_time = MissionTime::Paused(time::Duration::zero());
            })
            .set(MISSION_RESET_BUTTON, ui);

        // Time delay
        Label::new("Time Delay: 0s")
            .xy((-ui.win_w / 2.0) + 70.0, (ui.win_h / 2.0) - 150.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(TIME_DELAY, ui);

        ////////////////////////////////////////////////////////////////////////////////////////////
        // IMU section

        Label::new("IMU")
            .xy((-ui.win_w / 2.0) + 100.0, (ui.win_h / 2.0) - 190.0)
            .font_size(22)
            .color(self.bg_color.plain_contrast())
            .set(IMU_LABEL, ui);

        let (pitch, roll, heading, imu_color) =
            match self.pitch_roll_heading {
                Some((pitch, roll, heading)) => (format!("{0:.1}", pitch),
                                                 format!("{0:.1}", roll),
                                                 format!("{0:.1}", heading),
                                                 rgb(0.0, 1.0, 0.0)),
                None => ("NO DATA".to_string(), "NO DATA".to_string(),
                         "NO DATA".to_string(), rgb(1.0, 0.0, 0.0)),
            };

        // IMU pitch

        Label::new(format!("Pitch").as_str())
            .xy((-ui.win_w / 2.0) + 40.0, (ui.win_h / 2.0) - 350.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(IMU_PITCH_LABEL, ui);

        Label::new(pitch.as_str())
            .xy((-ui.win_w / 2.0) + 120.0, (ui.win_h / 2.0) - 350.0)
            .font_size(16)
            .color(imu_color)
            .set(IMU_PITCH_VALUE, ui);

        // IMU roll

        Label::new(format!("Roll").as_str())
            .xy((-ui.win_w / 2.0) + 190.0, (ui.win_h / 2.0) - 350.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(IMU_ROLL_LABEL, ui);

        self.roll.draw(c.trans(170.0, 215.0), gl);

        Label::new(roll.as_str())
            .xy((-ui.win_w / 2.0) + 250.0, (ui.win_h / 2.0) - 350.0)
            .font_size(16)
            .color(imu_color)
            .set(IMU_ROLL_VALUE, ui);

        // IMU heading

        Label::new("Heading")
            .xy((-ui.win_w / 2.0) + 340.0, (ui.win_h / 2.0) - 350.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(IMU_HEADING_LABEL, ui);

        self.heading.draw(c.trans(320.0, 215.0), gl);

        Label::new(heading.as_str())
            .xy((-ui.win_w / 2.0) + 420.0, (ui.win_h / 2.0) - 350.0)
            .font_size(16)
            .color(imu_color)
            .set(IMU_HEADING_VALUE, ui);

        ////////////////////////////////////////////////////////////////////////////////////////////
        // GPS section

        Label::new("GPS")
            .xy((-ui.win_w / 2.0) + 50.0, (ui.win_h / 2.0) - 400.0)
            .font_size(22)
            .color(self.bg_color.plain_contrast())
            .set(GPS_LABEL, ui);

        // Latitude label
        let (latitude, latitude_color) =
            match self.latitude {
                Some(lat) => {
                    (format!("{0:.2} N", lat), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0)),
            };
        Label::new(latitude.as_str())
            .xy((-ui.win_w / 2.0) + 50.0, (ui.win_h / 2.0) - 425.0)
            .font_size(16)
            .color(latitude_color)
            .set(LATITUDE_LABEL, ui);

        // Longitude label
        let (longitude, longitude_color) =
            match self.longitude {
                Some(lng) => {
                    (format!("{0:.2} W", lng), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0)),
            };
        Label::new(longitude.as_str())
            .xy((-ui.win_w / 2.0) + 50.0, (ui.win_h / 2.0) - 445.0)
            .font_size(16)
            .color(longitude_color)
            .set(LONGITUDE_LABEL, ui);

        // Speed label
        let (speed, speed_color) =
            match self.speed {
                Some(speed) => {
                    (format!("{0:.2} m/s", speed), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0)),
            };
        Label::new(speed.as_str())
            .xy((-ui.win_w / 2.0) + 50.0, (ui.win_h / 2.0) - 465.0)
            .font_size(16)
            .color(speed_color)
            .set(SPEED_LABEL, ui);

        // Altitude label
        let (altitude, altitude_color) =
            match self.altitude {
                Some(alt) => {
                    (format!("{0:.2} m", alt), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0)),
            };
        Label::new(altitude.as_str())
            .xy((-ui.win_w / 2.0) + 50.0, (ui.win_h / 2.0) - 485.0)
            .font_size(16)
            .color(altitude_color)
            .set(ALTITUDE_LABEL, ui);

        // Angle label
        let (angle, angle_color) =
            match self.angle {
                Some(angle) => {
                    (format!("{0:.2} deg", angle), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0)),
            };
        Label::new(angle.as_str())
            .xy((-ui.win_w / 2.0) + 50.0, (ui.win_h / 2.0) - 505.0)
            .font_size(16)
            .color(angle_color)
            .set(ANGLE_LABEL, ui);

        ////////////////////////////////////////////////////////////////////////////////////////////

        // Left RPM slider
        Slider::new(self.l_rpm, -self.max_rpm, self.max_rpm)
            .dimensions(150.0, 30.0)
            .xy(250.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 410.0)
            .rgb(0.5, 0.3, 0.6)
            .frame(1.0)
            .label("L Motor")
            .label_color(white())
            .react(|new_rpm| {
                self.try_update_l_rpm(new_rpm);
            })
            .set(L_RPM_SLIDER, ui);

        // Right RPM slider
        Slider::new(self.r_rpm, -self.max_rpm, self.max_rpm)
            .dimensions(150.0, 30.0)
            .xy(250.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 450.0)
            .rgb(0.5, 0.3, 0.6)
            .frame(1.0)
            .label("R Motor")
            .label_color(white())
            .react(|new_rpm| {
                self.try_update_r_rpm(new_rpm);
            })
            .set(R_RPM_SLIDER, ui);

        // Stop button
        Button::new()
            .dimensions(100.0, 30.0)
            .xy(250.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 490.0)
            .rgb(1.0, 0.0, 0.0)
            .frame(1.0)
            .label("Stop")
            .react(|| {
                self.l_rpm = 0.0;
                self.r_rpm = 0.0;
                self.send_l_rpm();
                self.send_r_rpm();
                self.send_brake();
            })
            .set(STOP_BUTTON, ui);

        // Motor speed slider
        Slider::new(self.motor_speed, 0.0, 1.0)
            .dimensions(150.0, 30.0)
            .xy(420.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 425.0)
            .rgb(0.5, 0.3, 0.6)
            .frame(1.0)
            .label("Motor Speed")
            .label_color(white())
            .react(|new_speed| {
                self.motor_speed = new_speed;
            })
            .set(MOTOR_SPEED_SLIDER, ui);
        
        // Camera pan slider
        Slider::new(self.f_pan, 0.0, 180.0)
            .dimensions(150.0, 30.0)
            .xy((ui.win_w / 2.0) - 425.0, (ui.win_h / 2.0) - 425.0)
            .rgb(0.5, 0.3, 0.6)
            .frame(1.0)
            .label("Pan")
            .label_color(white())
            .react(|new_pan| {
                self.try_update_f_pan(new_pan);
            })
            .set(F_PAN_SLIDER, ui);

        // Camera tilt slider
        Slider::new(self.f_tilt, 90.0, 180.0)
            .dimensions(150.0, 30.0)
            .xy((ui.win_w / 2.0) - 270.0, (ui.win_h / 2.0) - 425.0)
            .rgb(0.5, 0.3, 0.6)
            .frame(1.0)
            .label("Tilt")
            .label_color(white())
            .react(|new_tilt| {
                self.try_update_f_tilt(new_tilt);
            })
            .set(F_TILT_SLIDER, ui);

        ////////////////////////////////////////////////////////////////////////////////////////////
        // SADL
        Label::new("SADL")
            .xy(50.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 540.0)
            .font_size(22)
            .color(self.bg_color.plain_contrast())
            .set(SADL_LABEL, ui);
        Button::new()
            .xy(120.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 540.0)
            .dimensions(60.0, 30.0)
            .rgb(0.3, 0.8, 0.3)
            .frame(1.0)
            .label("Up")
            .react(|| { self.sadl = 100.0; self.send_sadl(); })
            .set(SADL_UP, ui);
        Button::new()
            .xy(185.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 540.0)
            .dimensions(60.0, 30.0)
            .rgb(0.3, 0.8, 0.3)
            .frame(1.0)
            .label("Down")
            .react(|| { self.sadl = -100.0; self.send_sadl(); })
            .set(SADL_DOWN, ui);

        ////////////////////////////////////////////////////////////////////////////////////////////
        // Blade
        Label::new("Blade")
            .xy(300.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 540.0)
            .font_size(22)
            .color(self.bg_color.plain_contrast())
            .set(BLADE_LABEL, ui);
        Button::new()
            .xy(370.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 540.0)
            .dimensions(60.0, 30.0)
            .rgb(0.3, 0.8, 0.3)
            .frame(1.0)
            .label("Up")
            .react(|| { self.blade = 100.0; self.send_blade(); })
            .set(BLADE_UP, ui);
        Button::new()
            .xy(435.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 540.0)
            .dimensions(60.0, 30.0)
            .rgb(0.3, 0.8, 0.3)
            .frame(1.0)
            .label("Down")
            .react(|| { self.blade = -100.0; self.send_blade(); })
            .set(BLADE_DOWN, ui);

        ////////////////////////////////////////////////////////////////////////////////////////////
        // Command section
        Label::new("Command")
            .xy(110.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 580.0)
            .font_size(22)
            .color(self.bg_color.plain_contrast())
            .set(COMMAND_LABEL, ui);

        let mut should_send_command = false;
        TextBox::new(&mut self.command)
            .enabled(self.command_mode)
            .font_size(16)
            .dimensions(320.0, 20.0)
            .xy(165.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 605.0)
            .frame(1.0)
            .frame_color(self.bg_color.invert().plain_contrast())
            .color(self.bg_color.invert())
            .react(|_string: &mut String| { should_send_command = true; })
            .set(COMMAND_INPUT, ui);
        if should_send_command { self.send_command(); }

        Button::new()
            .dimensions(100.0, 30.0)
            .xy(380.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 605.0)
            .rgb(0.3, 0.8, 0.3)
            .frame(1.0)
            .label("Send")
            .react(|| { self.send_command(); })
            .set(SEND_COMMAND_BUTTON, ui);

        let mode_label =
            match self.command_mode {
                true  => "Command Mode",
                false => "Real-time Mode",
            };
        Label::new(mode_label)
            .xy(200.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 640.0)
            .font_size(22)
            .color(self.bg_color.plain_contrast())
            .set(MODE_LABEL, ui);
        Button::new()
            .dimensions(150.0, 30.0)
            .xy(380.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 640.0)
            .rgb(0.3, 0.8, 0.3)
            .frame(1.0)
            .label("Toggle Mode")
            .react(|| { self.command_mode = !self.command_mode; })
            .set(MODE_TOGGLE_BUTTON, ui);

        // Draw our UI!
        ui.draw(c, gl);
    }

    pub fn handle_packet(&mut self, packet: String) {
        //println!("{}", packet);

        let packets = packet.split("|");

        for packet in packets {
            let packet_parts: Vec<String> = packet.split(":").map(|s| s.to_string()).collect();

            match packet_parts[0].as_str() {
                "GPS" => {
                    if packet_parts.len() == 6 {
                        self.latitude = packet_parts[1].parse().ok();
                        self.longitude = packet_parts[2].parse().ok();
                        self.speed = packet_parts[3].parse().ok();
                        self.altitude = packet_parts[4].parse().ok();
                        self.angle = packet_parts[5].parse().ok();
                    }
                },
                "IMU" => {
                    let ax: f64 = packet_parts[1].parse().unwrap();
                    let ay: f64 = packet_parts[2].parse().unwrap();
                    let az: f64 = packet_parts[3].parse().unwrap();

                    let mx: f64 = packet_parts[7].parse().unwrap();
                    let my: f64 = packet_parts[8].parse().unwrap();
                    let mz: f64 = packet_parts[9].parse().unwrap();

                    let roll = f64::atan2(ay, az);
                    let pitch = f64::atan2(-ax, ay*f64::sin(roll) + az*f64::cos(roll));
                    let heading = f64::atan2(mz*f64::sin(roll) - my*f64::cos(roll),
                                             mx*f64::cos(pitch) + my*f64::sin(pitch)*f64::sin(roll) + mz*f64::sin(pitch)*f64::cos(roll));

                    let mut heading = heading.to_degrees();
                    if heading < 0.0 {
                        heading += 360.0;
                    }
                    heading = 360.0 - heading;
                    self.pitch_roll_heading = Some((pitch.to_degrees(), roll.to_degrees(), heading));
                    self.roll.set_angle(roll);
                    self.heading.set_angle(heading);
                },
                _ => { /*println!("WARNING: Unknown packet ID: {}", packet_parts[0])*/ },
            }
        }
    }

    pub fn on_key_pressed<'a>(&mut self, key: input::Key) {
        use piston::input::Key::*;

        if self.command_mode {
            return;
        }

        // here need to add key for rpm values, need stuff between 0 and 100 - 10/29 CP
        // thought was to have '+' and '-' keys control a percentage slider, where
        // the l_rpm and r_rpm get multiplied by this perecentage (1 for 100%, 0.5 for 50%)
        // so that controls stay the same, only get multiplied by this variable


        match key {
            Space => {
                // LR motor stop
                self.l_rpm = 0.0;
                self.r_rpm = 0.0;
                self.send_l_rpm();
                self.send_r_rpm();
                // Brake
                self.send_brake();
            }
            Up => {
                // Forward
                self.l_rpm = 100.0*self.motor_speed;
                self.r_rpm = 100.0*self.motor_speed;
                self.send_l_rpm();
                self.send_r_rpm();
            },
            Down => {
                // Forward
                self.l_rpm = -100.0*self.motor_speed;
                self.r_rpm = -100.0*self.motor_speed;
                self.send_l_rpm();
                self.send_r_rpm();
            },
            Left => {
                // Forward
                self.l_rpm = -100.0*self.motor_speed;
                self.r_rpm = 100.0*self.motor_speed;
                self.send_l_rpm();
                self.send_r_rpm();
            },
            Right => {
                // Forward
                self.l_rpm = 100.0*self.motor_speed;
                self.r_rpm = -100.0*self.motor_speed;
                self.send_l_rpm();
                self.send_r_rpm();
            },
            Minus => {
                self.motor_speed -= 0.1;
            },
            Equals => {
                self.motor_speed += 0.1;
            },
            D1 => {
                // SADL up
                self.sadl = 100.0;
                self.send_sadl();
            },
            D2 => {
                // SADL down
                self.sadl = -100.0;
                self.send_sadl();
            },
            D9 => {
                // Blade up
                self.blade = 100.0;
                self.send_blade();
            },
            D0 => {
                // Blade down
                self.blade = -100.0;
                self.send_blade();
            },
            W => {
                // Camera up
                self.f_tilting = 1.0;
            },
            S => {
                // Camera down
                self.f_tilting = -1.0;
            },
            A => {
                // Camera left
                self.f_panning = -1.0;
            },
            D => {
                // Camera right
                self.f_panning = 1.0;
            },
            _ => { },
        }
    }

    pub fn on_key_released<'a>(&mut self, key: input::Key) {
        use piston::input::Key::*;

        if self.command_mode {
            return;
        }

        match key {
            Up | Down | Left | Right => {
                // LR motor stop
                self.l_rpm = 0.0;
                self.r_rpm = 0.0;
                self.send_l_rpm();
                self.send_r_rpm();
            },
            D1 | D2 => {
                // SADL stop
                self.sadl = 0.0;
                self.send_sadl();
            },
            D9 | D0 => {
                // Blade stop
                self.blade = 0.0;
                self.send_blade();
            },
            W | S => {
                self.f_tilting = 0.0;
                self.send_f_tilt();
            },
            A | D => {
                self.f_panning = 0.0;
                self.send_f_pan();
            },
            _ => { },
        }
    }

    pub fn try_update_l_rpm(&mut self, l_rpm: f32) -> io::Result<usize> {
        if (l_rpm - self.l_rpm).abs() > 5.0 {
            self.l_rpm = l_rpm;
            self.send_l_rpm()
        } else {
            Ok(0)
        }
    }

    pub fn try_update_r_rpm(&mut self, r_rpm: f32) -> io::Result<usize> {
        if (r_rpm - self.r_rpm).abs() > 5.0 {
            self.r_rpm = r_rpm;
            self.send_r_rpm()
        } else {
            Ok(0)
        }
    }

    pub fn try_update_f_pan(&mut self, f_pan: f32) -> io::Result<usize> {
        if (f_pan - self.f_pan).abs() > 5.0 || f_pan == 0.0 || f_pan == 180.0 {
            self.f_pan = f_pan;
            self.send_f_pan()
        } else {
            Ok(0)
        }
    }

    pub fn try_update_f_tilt(&mut self, f_tilt: f32) -> io::Result<usize> {
        if (f_tilt - self.f_tilt).abs() > 5.0 || f_tilt == 90.0 || f_tilt == 180.0 {
            self.f_tilt = f_tilt;
            self.send_f_tilt()
        } else {
            Ok(0)
        }
    }

    pub fn try_update_sadl(&mut self, sadl: f32) -> io::Result<usize> {
        if sadl != self.sadl && ((sadl - self.sadl).abs() > 5.0 || sadl == 0.0 || sadl == 100.0) {
            self.sadl = sadl;
            self.send_sadl()
        } else {
            Ok(0)
        }
    }

    pub fn send_brake(&self) -> io::Result<usize> {
        self.socket.send_to(&[b'G'], ("10.10.155.165", 30001))
    }

    pub fn send_l_rpm(&self) -> io::Result<usize> {
        let packet = format!("A{}", self.l_rpm as i32);
        self.socket.send_to(packet.as_bytes(), ("10.10.155.165", 30001))
    }

    pub fn send_r_rpm(&self) -> io::Result<usize> {
        let packet = format!("B{}", self.r_rpm as i32);
        self.socket.send_to(packet.as_bytes(), ("10.10.155.165", 30001))
    }

    pub fn send_f_pan(&mut self) -> io::Result<usize> {
        let time_since = (time::now() - self.last_f_pan_time).num_milliseconds();
        if time_since >= 500 {
            self.last_f_pan_time = time::now();
            let packet = format!("C{}", self.f_pan as i32);
            self.socket.send_to(packet.as_bytes(), ("10.10.155.165", 30001))
        } else {
            Ok(0)
        }
    }

    pub fn send_f_tilt(&mut self) -> io::Result<usize> {
        let time_since = (time::now() - self.last_f_tilt_time).num_milliseconds();
        if time_since >= 500 {
            self.last_f_tilt_time = time::now();
            let packet = format!("D{}", self.f_tilt as i32);
            self.socket.send_to(packet.as_bytes(), ("10.10.155.165", 30001))
        } else {
            Ok(0)
        }
    }

    pub fn send_sadl(&self) -> io::Result<usize> {
        let packet = format!("E{}", self.sadl as i32);
        self.socket.send_to(packet.as_bytes(), ("10.10.155.165", 30001))
    }

    pub fn send_blade(&self) -> io::Result<usize> {
        let packet = format!("F{}", self.blade as i32);
        self.socket.send_to(packet.as_bytes(), ("10.10.155.165", 30001))
    }

    pub fn send_command(&self) -> io::Result<usize> {
        let packet = format!("Z{}", self.command);
        self.socket.send_to(packet.as_bytes(), ("10.10.155.165", 30001))
    }

    pub fn send_packet(&mut self, delay: time::Duration, data: Vec<u8>, addr: (String, u16)) {
        self.out_queue.push_back((time::now(), delay, data, addr));
    }

    fn flush_out_queue(&mut self) {
        while !self.out_queue.is_empty() {
            if time::now()-self.out_queue[0].0 >= self.out_queue[0].1 {
                let (_, _, data, addr) = self.out_queue.pop_front().unwrap();
                self.socket.send_to(data.as_slice(), (addr.0.as_str(), addr.1));
            } else {
                break;
            }
        }
    }
}

// Widget IDs
const LOCAL_TIME: WidgetId = 0;
const UTC_TIME: WidgetId = LOCAL_TIME + 1;
const MISSION_TIME_LABEL: WidgetId = UTC_TIME + 1;
const MISSION_START_BUTTON: WidgetId = MISSION_TIME_LABEL + 1;
const MISSION_RESET_BUTTON: WidgetId = MISSION_START_BUTTON + 1;
const TIME_DELAY: WidgetId = MISSION_RESET_BUTTON + 1;

// IMU section
const IMU_LABEL: WidgetId = TIME_DELAY + 1;

const IMU_PITCH_LABEL: WidgetId = IMU_LABEL + 1;
const IMU_PITCH_VALUE: WidgetId = IMU_PITCH_LABEL + 1;

const IMU_ROLL_LABEL: WidgetId = IMU_PITCH_VALUE + 1;
const IMU_ROLL_VALUE: WidgetId = IMU_ROLL_LABEL + 1;

const IMU_HEADING_LABEL: WidgetId = IMU_ROLL_VALUE + 1;
const IMU_HEADING_VALUE: WidgetId = IMU_HEADING_LABEL + 1;

// GPS section
const GPS_LABEL: WidgetId = IMU_HEADING_VALUE + 1;
const LATITUDE_LABEL: WidgetId = GPS_LABEL + 1;
const LONGITUDE_LABEL: WidgetId = LATITUDE_LABEL + 1;
const SPEED_LABEL: WidgetId = LONGITUDE_LABEL + 1;
const ALTITUDE_LABEL: WidgetId = SPEED_LABEL + 1;
const ANGLE_LABEL: WidgetId = ALTITUDE_LABEL + 1;

const L_RPM_SLIDER: WidgetId = ANGLE_LABEL + 1;
const R_RPM_SLIDER: WidgetId = L_RPM_SLIDER + 1;
const MOTOR_SPEED_SLIDER: WidgetId = R_RPM_SLIDER+ 1;
const STOP_BUTTON: WidgetId = MOTOR_SPEED_SLIDER + 1;
const F_PAN_SLIDER: WidgetId = STOP_BUTTON + 1;
const F_TILT_SLIDER: WidgetId = F_PAN_SLIDER + 1;
const COMMAND_LABEL: WidgetId = F_TILT_SLIDER + 1;
const COMMAND_INPUT: WidgetId = COMMAND_LABEL + 1;
const SEND_COMMAND_BUTTON: WidgetId = COMMAND_INPUT + 1;
const MODE_LABEL: WidgetId = SEND_COMMAND_BUTTON + 1;
const MODE_TOGGLE_BUTTON: WidgetId = MODE_LABEL + 1;

const SADL_LABEL: WidgetId = MODE_TOGGLE_BUTTON + 1;
const SADL_UP: WidgetId = SADL_LABEL + 1;
const SADL_DOWN: WidgetId = SADL_UP + 1;

const BLADE_LABEL: WidgetId = SADL_DOWN + 1;
const BLADE_UP: WidgetId = BLADE_LABEL + 1;
const BLADE_DOWN: WidgetId = BLADE_UP + 1;
