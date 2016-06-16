use std::collections::{HashMap, VecDeque};
use std::io;
use std::io::Write;
use std::net::UdpSocket;
use std::ops::DerefMut;
use std::sync::mpsc::Sender;

use conrod::{
    self,
    Background,
    Button,
    Color,
    Colorable,
    Frameable,
    Text,
    Labelable,
    Positionable,
    Slider,
    Sizeable,
    TextBox,
    Ui,
    Widget,
};
use conrod::color::{rgb, WHITE};
use graphics::{Context, Graphics};
use piston_window::{self, Key};
use time;

use conrod_config;
use imu;
use video_stream::VideoMsg;

enum MissionTime {
    Paused(time::Duration),
    Running(time::Tm, time::Duration),
}

pub struct StereoUi {
    bg_color: Color,

    mission_time: MissionTime,

    // IMU
    pitch_roll_heading: Option<(f64, f64, f64)>,
    pitch: imu::Roll,
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
    pub pan: f32,
    pub panning: f32,
    pub last_pan_time: time::Tm,
    pub tilt: f32,
    pub tilting: f32,
    pub last_tilt_time: time::Tm,

    pub command: String,
    pub command_mode: bool,

    client: UdpSocket,
    vid0_t: Sender<VideoMsg>,
    vid1_t: Sender<VideoMsg>,
    vid2_t: Sender<VideoMsg>,
    mission_folder: String,
    vid_num: u16,

    out_queue: VecDeque<(time::Tm, time::Duration, Vec<u8>, (String, u16))>, // Outbound packet queue
    delay: time::Duration,
    delay_str: String,
}

impl StereoUi {
    pub fn new(client: UdpSocket,
               vid0_t: Sender<VideoMsg>,
               vid1_t: Sender<VideoMsg>,
               vid2_t: Sender<VideoMsg>,
               mission_folder: String) -> StereoUi {
        StereoUi {
            bg_color: rgb(0.2, 0.35, 0.45),

            mission_time: MissionTime::Paused(time::Duration::zero()),

            pitch_roll_heading: None,
            pitch: imu::Roll::new(),
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

            pan: 90.0,
            panning: 0.0,
            last_pan_time: time::now(),
            tilt: 90.0,
            tilting: 0.0,
            last_tilt_time: time::now(),

            command: "".to_string(),
            command_mode: false,

            client: client,
            vid0_t: vid0_t,
            vid1_t: vid1_t,
            vid2_t: vid2_t,
            mission_folder: mission_folder,
            vid_num: 0,

            out_queue: VecDeque::new(),
            delay: time::Duration::seconds(0),
            delay_str: "".to_string(),
        }
    }

    pub fn update(&mut self, dt: f64) {
        let dt = dt as f32;

        self.pan += self.panning*180.0*dt; // 180 degrees per second
        self.tilt += self.tilting*90.0*dt; // 90 degrees per second

        self.flush_out_queue();
    }

    pub fn draw_ui<'a, G>(&mut self, c: Context, g: &mut G, ui: &mut conrod_config::Ui)
                          where G: Graphics<Texture=<piston_window::G2d<'static> as conrod::Graphics>::Texture> {
        use graphics::Transformed;

        // Draw the background.
        Background::new().color(self.bg_color).set(ui);

        // Draw our UI!
        ui.draw(c, g);
    }

    pub fn set_widgets(&mut self, ui: &mut conrod_config::UiCell) {
        let time_now = time::now();

        // Local time
        Text::new(format!("{}", time_now.strftime("Local  %x  %X").unwrap()).as_str())
            .x_y((-ui.win_w / 2.0) + 100.0, (ui.win_h / 2.0) - 10.0)
            .font_size(16)
            .color(self.bg_color.plain_contrast())
            .set(LOCAL_TIME, ui);

        // UTC time
        Text::new(format!("{}", time_now.to_utc().strftime("%Z  %x  %X").unwrap()).as_str())
            .x_y((-ui.win_w / 2.0) + 104.0, (ui.win_h / 2.0) - 30.0)
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
        Text::new(format!("Mission Time: {}:{}:{}:{}", days, hours, minutes, seconds).as_str())
            .x_y((-ui.win_w / 2.0) + 150.0, (ui.win_h / 2.0) - 70.0)
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
            .w_h(100.0, 30.0)
            .x_y((-ui.win_w / 2.0) + 55.0, (ui.win_h / 2.0) - 100.0)
            .rgb(0.3, 0.8, 0.3)
            .frame(1.0)
            .label(mission_start_text)
            .react(|| {
                match self.mission_time {
                    MissionTime::Paused(current_time) => {
                        self.mission_time = MissionTime::Running(time::now(), current_time);

                        self.vid0_t.send(VideoMsg::Start(format!("mission_data/{}/forward{}.mp4", self.mission_folder, self.vid_num)));
                        self.vid1_t.send(VideoMsg::Start(format!("mission_data/{}/reverse{}.mkv", self.mission_folder, self.vid_num)));
                        self.vid2_t.send(VideoMsg::Start(format!("mission_data/{}/hazard{}.mkv", self.mission_folder, self.vid_num)));

                        self.vid_num += 1;
                    },
                    MissionTime::Running(start_time, extra_time) => {
                        self.mission_time = MissionTime::Paused((time::now() - start_time) + extra_time);

                        self.vid0_t.send(VideoMsg::Stop);
                        self.vid1_t.send(VideoMsg::Stop);
                        self.vid2_t.send(VideoMsg::Stop);
                    },
                };
            })
            .set(MISSION_START_BUTTON, ui);

        // Mission reset button
        Button::new()
            .w_h(100.0, 30.0)
            .x_y((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 100.0)
            .rgb(0.3, 0.8, 0.3)
            .frame(1.0)
            .label("Reset")
            .react(|| {
                self.mission_time = MissionTime::Paused(time::Duration::zero());
            })
            .set(MISSION_RESET_BUTTON, ui);

        // Time delay
        Text::new("Time Delay:")
            .x_y((-ui.win_w / 2.0) + 70.0, (ui.win_h / 2.0) - 150.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(TIME_DELAY, ui);

        let mut new_delay = false;
        TextBox::new(&mut self.delay_str)
            .font_size(16)
            .w_h(50.0, 20.0)
            .x_y((-ui.win_w / 2.0) + 150.0, (ui.win_h / 2.0) - 150.0)
            .frame(1.0)
            .frame_color(self.bg_color.invert().plain_contrast())
            .color(self.bg_color.invert())
            .react(|s: &mut String| {
                new_delay = true;
            })
            .set(TIME_DELAY_VALUE, ui);
        if new_delay {
            self.delay = time::Duration::seconds(self.delay_str.parse().unwrap());
        }

        ////////////////////////////////////////////////////////////////////////////////////////////
        
        // Camera pan slider
        Slider::new(self.pan, 0.0, 180.0)
            .w_h(150.0, 30.0)
            .x_y((ui.win_w / 2.0) - 425.0, (ui.win_h / 2.0) - 425.0)
            .rgb(0.5, 0.3, 0.6)
            .frame(1.0)
            .label("Pan")
            .label_color(WHITE)
            .react(|new_pan| {
                self.try_update_pan(new_pan);
            })
            .set(F_PAN_SLIDER, ui);

        // Camera tilt slider
        Slider::new(self.tilt, 0.0, 180.0)
            .w_h(150.0, 30.0)
            .x_y((ui.win_w / 2.0) - 270.0, (ui.win_h / 2.0) - 425.0)
            .rgb(0.5, 0.3, 0.6)
            .frame(1.0)
            .label("Tilt")
            .label_color(WHITE)
            .react(|new_tilt| {
                self.try_update_tilt(new_tilt);
            })
            .set(F_TILT_SLIDER, ui);

        ////////////////////////////////////////////////////////////////////////////////////////////
       
        // Command section
        Text::new("Command")
            .x_y(110.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 580.0)
            .font_size(22)
            .color(self.bg_color.plain_contrast())
            .set(COMMAND_LABEL, ui);

        let mut should_send_command = false;
        TextBox::new(&mut self.command)
            .enabled(self.command_mode)
            .font_size(16)
            .w_h(320.0, 20.0)
            .x_y(165.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 605.0)
            .frame(1.0)
            .frame_color(self.bg_color.invert().plain_contrast())
            .color(self.bg_color.invert())
            .react(|_string: &mut String| { should_send_command = true; })
            .set(COMMAND_INPUT, ui);
        if should_send_command { self.send_command(); }

        Button::new()
            .w_h(100.0, 30.0)
            .x_y(380.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 605.0)
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
        Text::new(mode_label)
            .x_y(200.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 640.0)
            .font_size(22)
            .color(self.bg_color.plain_contrast())
            .set(MODE_LABEL, ui);
        Button::new()
            .w_h(150.0, 30.0)
            .x_y(380.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 640.0)
            .rgb(0.3, 0.8, 0.3)
            .frame(1.0)
            .label("Toggle Mode")
            .react(|| { self.command_mode = !self.command_mode; })
            .set(MODE_TOGGLE_BUTTON, ui);
    }

    pub fn handle_packet(&mut self, packet: String) {
        //println!("{}", packet);

        let packets = packet.split("|");

        for packet in packets {
            let packet_parts: Vec<String> = packet.split(":").map(|s| s.to_string()).collect();

            match packet_parts[0].as_str() {
                _ => { /*println!("WARNING: Unknown packet ID: {}", packet_parts[0])*/ },
            }
        }
    }

    pub fn on_key_pressed<'a>(&mut self, key: Key) {
        use piston_window::Key::*;

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
                self.send_lr_rpm();
                // Brake
                self.send_brake();
            }
            Up => {
                // Forward
                println!("foo");
                self.l_rpm = 100.0*self.motor_speed;
                self.r_rpm = 100.0*self.motor_speed;
                self.send_lr_rpm();
            },
            Down => {
                // Forward
                self.l_rpm = -100.0*self.motor_speed;
                self.r_rpm = -100.0*self.motor_speed;
                self.send_lr_rpm();
            },
            Left => {
                // Forward
                self.l_rpm = -100.0*self.motor_speed;
                self.r_rpm = 100.0*self.motor_speed;
                self.send_lr_rpm();
            },
            Right => {
                // Forward
                self.l_rpm = 100.0*self.motor_speed;
                self.r_rpm = -100.0*self.motor_speed;
                self.send_lr_rpm();
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
                self.tilting = 1.0;
            },
            S => {
                // Camera down
                self.tilting = -1.0;
            },
            A => {
                // Camera left
                self.panning = -1.0;
            },
            D => {
                // Camera right
                self.panning = 1.0;
            },
            _ => { },
        }
    }

    pub fn on_key_released<'a>(&mut self, key: Key) {
        use piston_window::Key::*;

        if self.command_mode {
            return;
        }

        match key {
            Up | Down | Left | Right => {
                // LR motor stop
                self.l_rpm = 0.0;
                self.r_rpm = 0.0;
                self.send_lr_rpm();
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
                self.tilting = 0.0;
                self.send_tilt();
            },
            A | D => {
                self.panning = 0.0;
                self.send_pan();
            },
            _ => { },
        }
    }

    pub fn try_update_l_rpm(&mut self, l_rpm: f32) {
        if (l_rpm - self.l_rpm).abs() > 5.0 {
            self.l_rpm = l_rpm;
            self.send_l_rpm();
        }
    }

    pub fn try_update_r_rpm(&mut self, r_rpm: f32) {
        if (r_rpm - self.r_rpm).abs() > 5.0 {
            self.r_rpm = r_rpm;
            self.send_r_rpm();
        }
    }

    pub fn try_update_pan(&mut self, pan: f32) {
        if (pan - self.pan).abs() > 5.0 || pan == 0.0 || pan == 180.0 {
            self.pan = pan;
            self.send_pan();
        }
    }

    pub fn try_update_tilt(&mut self, tilt: f32) {
        if (tilt - self.tilt).abs() > 5.0 || tilt == 90.0 || tilt == 180.0 {
            self.tilt = tilt;
            self.send_tilt();
        }
    }

    pub fn try_update_sadl(&mut self, sadl: f32) {
        if sadl != self.sadl && ((sadl - self.sadl).abs() > 5.0 || sadl == 0.0 || sadl == 100.0) {
            self.sadl = sadl;
            self.send_sadl();
        }
    }

    pub fn send_brake(&mut self) {
        let delay = self.delay;
        self.queue_packet(delay, vec![b'G'], ("10.10.155.165".to_string(), 30001));
    }

    pub fn send_l_rpm(&mut self) {
        let packet = format!("A{}|", self.l_rpm as i32);
        let delay = self.delay;
        self.queue_packet(delay, packet.into_bytes(), ("10.10.155.165".to_string(), 30001));
    }

    pub fn send_r_rpm(&mut self) {
        let packet = format!("B{}|", self.r_rpm as i32);
        let delay = self.delay;
        self.queue_packet(delay, packet.into_bytes(), ("10.10.155.165".to_string(), 30001));
    }

    pub fn send_lr_rpm(&mut self) {
        let packet = format!("H{}|{}|", self.l_rpm as i32, self.r_rpm as i32);
        let delay = self.delay;
        self.queue_packet(delay, packet.into_bytes(), ("10.10.155.165".to_string(), 30001));
    }

    pub fn send_pan(&mut self) {
        let time_since = (time::now() - self.last_pan_time).num_milliseconds();
        if time_since >= 500 {
            self.last_pan_time = time::now();
            let packet = format!("I{}|", self.pan as i32);
            let delay = self.delay;
            self.queue_packet(delay, packet.into_bytes(), ("10.10.155.165".to_string(), 30001));
        }
    }

    pub fn send_tilt(&mut self) {
        let time_since = (time::now() - self.last_tilt_time).num_milliseconds();
        if time_since >= 500 {
            self.last_tilt_time = time::now();
            let packet = format!("J{}|", self.tilt as i32);
            let delay = self.delay;
            self.queue_packet(delay, packet.into_bytes(), ("10.10.155.165".to_string(), 30001));
        }
    }

    pub fn send_sadl(&mut self) {
        let packet = format!("E{}|", self.sadl as i32);
        let delay = self.delay;
        self.queue_packet(delay, packet.into_bytes(), ("10.10.155.165".to_string(), 30001));
    }

    pub fn send_blade(&mut self) {
        let packet = format!("F{}|", self.blade as i32);
        let delay = self.delay;
        self.queue_packet(delay, packet.into_bytes(), ("10.10.155.165".to_string(), 30001));
    }

    pub fn send_command(&mut self) {
        let packet = format!("Z{}|{}|", self.command, self.motor_speed);
        let delay = self.delay;
        self.queue_packet(delay, packet.into_bytes(), ("10.10.155.165".to_string(), 30001));
    }

    pub fn queue_packet(&mut self, delay: time::Duration, mut data: Vec<u8>, addr: (String, u16)) {
        data.push(0); // Null terminate all of our packets
        self.out_queue.push_back((time::now(), delay, data, addr));
    }

    fn flush_out_queue(&mut self) -> io::Result<usize> {
        use std::iter;

        let mut bytes_written = 0;
        while !self.out_queue.is_empty() {
            if time::now()-self.out_queue[0].0 >= self.out_queue[0].1 {
                let (_, _, mut data, addr) = self.out_queue.pop_front().unwrap();
                let data_len = data.len();
                bytes_written += try!(self.client.send_to(data.as_slice(), (addr.0.as_str(), addr.1)));
                //data.extend(iter::repeat(b' ').take(64 - data_len)); // Pad the message to always be 64 bytes
                //bytes_written += try!(self.client.write(data.as_slice()));
            } else {
                break;
            }
        }
        Ok(bytes_written)
    }
}

widget_ids! {
    LOCAL_TIME,
    UTC_TIME,
    MISSION_TIME_LABEL,
    MISSION_START_BUTTON,
    MISSION_RESET_BUTTON,
    TIME_DELAY,
    TIME_DELAY_VALUE,

    F_PAN_SLIDER,
    F_TILT_SLIDER,
    COMMAND_LABEL,
    COMMAND_INPUT,
    SEND_COMMAND_BUTTON,
    MODE_LABEL,
    MODE_TOGGLE_BUTTON,

    SADL_LABEL,
    SADL_UP,
    SADL_DOWN,

    BLADE_LABEL,
    BLADE_UP,
    BLADE_DOWN,
}
