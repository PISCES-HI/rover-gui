use std::io;
use std::net::UdpSocket;
use std::ops::DerefMut;

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

enum MissionTime {
    Paused(time::Duration),
    Running(time::Tm, time::Duration),
}

pub struct NavigationUi {
    bg_color: Color,
    
    mission_time: MissionTime,

    // IMU
    pitch_roll_heading: Option<(f64, f64, f64)>,
    
    // RPM stuff
    pub l_rpm: f32,
    pub r_rpm: f32,
    pub max_rpm: f32,
    l_rpm_status: String,
    r_rpm_status: String,

    pub sadl: f32,
    pub blade: f32,
    
    // Forward camera controls
    pub f_pan: f32,
    pub f_tilt: f32,

    pub command: String,
    
    socket: UdpSocket,
}

impl NavigationUi {
    pub fn new(socket: UdpSocket) -> NavigationUi {
        NavigationUi {
            bg_color: rgb(0.2, 0.35, 0.45),
            
            mission_time: MissionTime::Paused(time::Duration::zero()),

            pitch_roll_heading: None,
            
            l_rpm: 0.0,
            r_rpm: 0.0,
            max_rpm: 100.0,
            l_rpm_status: "UNAVAILABLE".to_string(),
            r_rpm_status: "UNAVAILABLE".to_string(),

            sadl: 0.0,
            blade: 0.0,
            
            f_pan: 90.0,
            f_tilt: 130.0,

            command: "".to_string(),
            
            socket: socket,
        }
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
                    },
                    MissionTime::Running(start_time, extra_time) => {
                        self.mission_time = MissionTime::Paused((time::now() - start_time) + extra_time);
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
            .xy((-ui.win_w / 2.0) + 40.0, (ui.win_h / 2.0) - 300.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(IMU_PITCH_LABEL, ui);

        Label::new(pitch.as_str())
            .xy((-ui.win_w / 2.0) + 120.0, (ui.win_h / 2.0) - 300.0)
            .font_size(16)
            .color(imu_color)
            .set(IMU_PITCH_VALUE, ui);

        // IMU roll

        Label::new(format!("Roll").as_str())
            .xy((-ui.win_w / 2.0) + 190.0, (ui.win_h / 2.0) - 300.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(IMU_ROLL_LABEL, ui);

        Label::new(roll.as_str())
            .xy((-ui.win_w / 2.0) + 270.0, (ui.win_h / 2.0) - 300.0)
            .font_size(16)
            .color(imu_color)
            .set(IMU_ROLL_VALUE, ui);

        // IMU heading

        Label::new("Heading")
            .xy((-ui.win_w / 2.0) + 340.0, (ui.win_h / 2.0) - 300.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(IMU_HEADING_LABEL, ui);

        Label::new(heading.as_str())
            .xy((-ui.win_w / 2.0) + 420.0, (ui.win_h / 2.0) - 300.0)
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
        
        // Longitude label
        Label::new("19 43' 1\" N")
            .xy((-ui.win_w / 2.0) + 50.0, (ui.win_h / 2.0) - 425.0)
            .font_size(16)
            .color(self.bg_color.plain_contrast())
            .set(LONGITUDE_LABEL, ui);
        
        // Latitude label
        Label::new("155 4' 1\" W")
            .xy((-ui.win_w / 2.0) + 50.0, (ui.win_h / 2.0) - 445.0)
            .font_size(16)
            .color(self.bg_color.plain_contrast())
            .set(LATITUDE_LABEL, ui);
        
        // Longitude label
        Label::new("0.5 m/s")
            .xy((-ui.win_w / 2.0) + 50.0, (ui.win_h / 2.0) - 465.0)
            .font_size(16)
            .color(self.bg_color.plain_contrast())
            .set(VELOCITY_LABEL, ui);

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
            })
            .set(STOP_BUTTON, ui);
        
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

        // SADL slider
        Slider::new(self.sadl, -100.0, 100.0)
            .dimensions(150.0, 30.0)
            .xy(250.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 540.0)
            .rgb(0.5, 0.3, 0.6)
            .frame(1.0)
            .label("SADL")
            .label_color(white())
            .react(|new_sadl| {
                self.try_update_sadl(new_sadl);
            })
            .set(SADL_SLIDER, ui);

        // Command section
        Label::new("Command")
            .xy(110.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 580.0)
            .font_size(22)
            .color(self.bg_color.plain_contrast())
            .set(COMMAND_LABEL, ui);
        
        TextBox::new(&mut self.command)
            .font_size(16)
            .dimensions(320.0, 20.0)
            .xy(165.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 605.0)
            .frame(1.0)
            .frame_color(self.bg_color.invert().plain_contrast())
            .color(self.bg_color.invert())
            .react(|_string: &mut String|{})
            .set(COMMAND_INPUT, ui);

        Button::new()
            .dimensions(100.0, 30.0)
            .xy(380.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 605.0)
            .rgb(0.3, 0.8, 0.3)
            .frame(1.0)
            .label("Send")
            .react(|| { self.send_command(); })
            .set(SEND_COMMAND_BUTTON, ui);
        
        // Left status RPM
        /*Label::new(self.l_rpm_status.as_str())
            .xy(110.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 60.0)
            .font_size(32)
            .color(self.bg_color.plain_contrast())
            .set(L_RPM_STATUS, ui);
        
        // Right status RPM
        Label::new(self.r_rpm_status.as_str())
            .xy((ui.win_w / 2.0) - 110.0, (ui.win_h / 2.0) - 60.0)
            .font_size(32)
            .color(self.bg_color.plain_contrast())
            .set(R_RPM_STATUS, ui);
        
        // Blade slider
        Slider::new(self.blade, -10.0, 10.0)
            .dimensions(200.0, 30.0)
            .xy(110.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 160.0)
            .rgb(0.5, 0.3, 0.6)
            .frame(1.0)
            .label("Blade")
            .label_color(white())
            .react(|new_blade| {
                self.try_update_blade(new_blade);
            })
            .set(BLADE_SLIDER, ui);
        
        // 12v bus label
        Label::new("12v Bus")
            .xy((-ui.win_w / 2.0) + 100.0, (ui.win_h / 2.0) - 215.0)
            .font_size(32)
            .color(self.bg_color.plain_contrast())
            .set(VOLTAGE_12_LABEL, ui);
        */

        // Draw our UI!
        ui.draw(c, gl);
    }
    
    pub fn handle_packet(&mut self, packet: String) {
        //println!("Got packet: {}", packet);
        let packet_parts: Vec<String> = packet.split(":").map(|s| s.to_string()).collect();
        
        match packet_parts[0].as_str() {
            "RPM_STATUS" => {
                self.l_rpm_status = packet_parts[1].clone();
                self.r_rpm_status = packet_parts[2].clone();
            },
            "GPS" => {
                println!("{}", packet);
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
            },
            _ => { println!("WARNING: Unknown packet ID: {}", packet_parts[0]) },
        }
    }
    
    pub fn on_key_pressed(&mut self, key: input::Key) {
        use piston::input::Key::*;
        match key {
            W => {
                // Forward
                self.l_rpm = 100.0;
                self.r_rpm = 100.0;
                self.send_l_rpm();
                self.send_r_rpm();
            },
            S => {
                // Forward
                self.l_rpm = -100.0;
                self.r_rpm = -100.0;
                self.send_l_rpm();
                self.send_r_rpm();
            },
            A => {
                // Forward
                self.l_rpm = -100.0;
                self.r_rpm = 100.0;
                self.send_l_rpm();
                self.send_r_rpm();
            },
            D => {
                // Forward
                self.l_rpm = 100.0;
                self.r_rpm = -100.0;
                self.send_l_rpm();
                self.send_r_rpm();
            },
            R => {
                // SADL up
                self.sadl = 100.0;
                self.send_sadl();
            },
            F => {
                // SADL up
                self.sadl = -100.0;
                self.send_sadl();
            },
            Up => {
                // Camera up
                self.f_tilt += f32::min(5.0, 180.0 - self.f_tilt);
                self.send_f_tilt();
            },
            Down => {
                // Camera down
                self.f_tilt -= f32::min(5.0, self.f_tilt - 90.0);
                self.send_f_tilt();
            },
            Left => {
                // Camera up
                self.f_pan -= f32::min(5.0, self.f_pan - 0.0);
                self.send_f_pan();
            },
            Right => {
                // Camera down
                self.f_pan += f32::min(5.0, 180.0 - self.f_pan);
                self.send_f_pan();
            },
            _ => { },
        }
    }
    
    pub fn on_key_released(&mut self, key: input::Key) {
        use piston::input::Key::*;
        match key {
            W | S | A | D => {
                // LR motor stop
                self.l_rpm = 0.0;
                self.r_rpm = 0.0;
                self.send_l_rpm();
                self.send_r_rpm();
            },
            R | F => {
                // SADL stop
                self.sadl = 0.0;
                self.send_sadl();
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
        if (sadl - self.sadl).abs() > 5.0 || sadl == 0.0 || sadl == 100.0 {
            self.sadl = sadl;
            self.send_sadl()
        } else {
            Ok(0)
        }
    }
    
    pub fn send_l_rpm(&self) -> io::Result<usize> {
        let packet = format!("A{}", self.l_rpm as i32);
        self.socket.send_to(packet.as_bytes(), ("10.10.153.25", 30001))
    }
    
    pub fn send_r_rpm(&self) -> io::Result<usize> {
        let packet = format!("B{}", self.r_rpm as i32);
        self.socket.send_to(packet.as_bytes(), ("10.10.153.25", 30001))
    }
    
    pub fn send_f_pan(&self) -> io::Result<usize> {
        let packet = format!("C{}", self.f_pan as i32);
        self.socket.send_to(packet.as_bytes(), ("10.10.153.25", 30001))
    }
    
    pub fn send_f_tilt(&self) -> io::Result<usize> {
        let packet = format!("D{}", self.f_tilt as i32);
        self.socket.send_to(packet.as_bytes(), ("10.10.153.25", 30001))
    }

    pub fn send_sadl(&self) -> io::Result<usize> {
        let packet = format!("E{}", self.sadl as i32);
        self.socket.send_to(packet.as_bytes(), ("10.10.153.25", 30001))
    }

    pub fn send_command(&self) -> io::Result<usize> {
        let packet = format!("Z{}", self.command);
        self.socket.send_to(packet.as_bytes(), ("10.10.153.25", 30001))
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
const LONGITUDE_LABEL: WidgetId = GPS_LABEL + 1;
const LATITUDE_LABEL: WidgetId = LONGITUDE_LABEL + 1;
const VELOCITY_LABEL: WidgetId = LATITUDE_LABEL + 1;
const L_RPM_SLIDER: WidgetId = VELOCITY_LABEL + 1;
const R_RPM_SLIDER: WidgetId = L_RPM_SLIDER + 1;
const STOP_BUTTON: WidgetId = R_RPM_SLIDER + 1;
const F_PAN_SLIDER: WidgetId = STOP_BUTTON + 1;
const F_TILT_SLIDER: WidgetId = F_PAN_SLIDER + 1;
const SADL_SLIDER: WidgetId = F_TILT_SLIDER + 1;
const COMMAND_LABEL: WidgetId = SADL_SLIDER + 1;
const COMMAND_INPUT: WidgetId = COMMAND_LABEL + 1;
const SEND_COMMAND_BUTTON: WidgetId = COMMAND_INPUT + 1;

/*const L_RPM_STATUS: WidgetId = STOP_BUTTON + 1;
const R_RPM_STATUS: WidgetId = L_RPM_STATUS + 1;
const BLADE_SLIDER: WidgetId = R_RPM_STATUS + 1;
const VOLTAGE_12_LABEL: WidgetId = BLADE_SLIDER + 1;*/
