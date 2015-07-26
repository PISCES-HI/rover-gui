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

pub struct BladeUi {
    bg_color: Color,
    
    mission_time: MissionTime,
    
    pub blade: f32,
    
    socket: UdpSocket,
}

impl BladeUi {
    pub fn new(socket: UdpSocket) -> BladeUi {
        BladeUi {
            bg_color: rgb(0.2, 0.35, 0.45),
            
            mission_time: MissionTime::Paused(time::Duration::zero()),
            
            blade: 0.0,
            
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
        
        // IMU label
        Label::new("IMU")
            .xy((-ui.win_w / 2.0) + 100.0, (ui.win_h / 2.0) - 190.0)
            .font_size(22)
            .color(self.bg_color.plain_contrast())
            .set(IMU_LABEL, ui);
        
        // GPS label
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
        
        // Velocity label
        Label::new("0.5 m/s")
            .xy((-ui.win_w / 2.0) + 50.0, (ui.win_h / 2.0) - 465.0)
            .font_size(16)
            .color(self.bg_color.plain_contrast())
            .set(VELOCITY_LABEL, ui);

        // SADL slider
        Slider::new(self.blade, -100.0, 100.0)
            .dimensions(150.0, 30.0)
            .xy(250.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 520.0)
            .rgb(0.5, 0.3, 0.6)
            .frame(1.0)
            .label("BLADE")
            .label_color(white())
            .react(|new_blade| {
                self.try_update_blade(new_blade);
            })
            .set(SADL_SLIDER, ui);
        
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
        let packet_parts: Vec<String> = packet.split(":").map(|s| s.to_string()).collect();
        
        match packet_parts[0].as_str() {
            _ => { println!("WARNING: Unknown packet ID: {}", packet_parts[0]) },
        }
    }
    
    pub fn on_key_pressed(&mut self, key: input::Key) {
        match key {
            _ => { },
        }
    }
    
    pub fn on_key_released(&mut self, key: input::Key) {
        match key {
            _ => { },
        }
    }
    
    pub fn try_update_blade(&mut self, blade: f32) -> io::Result<usize> {
        if (blade - self.blade).abs() > 1.0 || blade == -10.0 || blade == 10.0 {
            self.blade = blade;
            self.send_blade()
        } else {
            Ok(0)
        }
    }
    
    pub fn send_blade(&self) -> io::Result<usize> {
        let packet = format!("F{}", self.blade as i32);
        self.socket.send_to(packet.as_bytes(), ("10.14.120.25", 30001))
    }
}

// Widget IDs
const LOCAL_TIME: WidgetId = 0;
const UTC_TIME: WidgetId = LOCAL_TIME + 1;
const MISSION_TIME_LABEL: WidgetId = UTC_TIME + 1;
const MISSION_START_BUTTON: WidgetId = MISSION_TIME_LABEL + 1;
const MISSION_RESET_BUTTON: WidgetId = MISSION_START_BUTTON + 1;
const TIME_DELAY: WidgetId = MISSION_RESET_BUTTON + 1;
const IMU_LABEL: WidgetId = TIME_DELAY + 1;
const GPS_LABEL: WidgetId = IMU_LABEL + 1;
const LONGITUDE_LABEL: WidgetId = GPS_LABEL + 1;
const LATITUDE_LABEL: WidgetId = LONGITUDE_LABEL + 1;
const VELOCITY_LABEL: WidgetId = LATITUDE_LABEL + 1;
const L_RPM_SLIDER: WidgetId = VELOCITY_LABEL + 1;
const R_RPM_SLIDER: WidgetId = L_RPM_SLIDER + 1;
const STOP_BUTTON: WidgetId = R_RPM_SLIDER + 1;
const F_PAN_SLIDER: WidgetId = STOP_BUTTON + 1;
const F_TILT_SLIDER: WidgetId = F_PAN_SLIDER + 1;
const SADL_SLIDER: WidgetId = F_TILT_SLIDER + 1;

/*const L_RPM_STATUS: WidgetId = STOP_BUTTON + 1;
const R_RPM_STATUS: WidgetId = L_RPM_STATUS + 1;
const BLADE_SLIDER: WidgetId = R_RPM_STATUS + 1;
const VOLTAGE_12_LABEL: WidgetId = BLADE_SLIDER + 1;*/
