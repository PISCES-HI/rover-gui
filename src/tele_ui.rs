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

use line_graph::LineGraph;

enum MissionTime {
    Paused(time::Duration),
    Running(time::Tm, time::Duration),
}

pub struct TelemetryUi {
    socket: UdpSocket,

    bg_color: Color,
    
    mission_time: MissionTime,
    
    // RPM stuff
    l_rpm_status: String,
    r_rpm_status: String,
    
    volt_graph_12: LineGraph,
    volts_12: String,
    amps_12: String,
}

impl TelemetryUi {
    pub fn new(socket: UdpSocket) -> TelemetryUi {
        let volt_graph_12 = LineGraph::new((200.0, 100.0), (0.0, 100.0), (0.0, 20.0));
    
        TelemetryUi {
            socket: socket,
        
            bg_color: rgb(0.2, 0.35, 0.45),
            
            mission_time: MissionTime::Paused(time::Duration::zero()),
            
            l_rpm_status: "UNAVAILABLE".to_string(),
            r_rpm_status: "UNAVAILABLE".to_string(),
            
            volt_graph_12: volt_graph_12,
            volts_12: "UNAVAILABLE".to_string(),
            amps_12: "UNAVAILABLE".to_string(),
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
        // Power section
        
        Label::new("Power")
            .xy((-ui.win_w / 2.0) + 110.0, (ui.win_h / 2.0) - 190.0)
            .font_size(20)
            .color(self.bg_color.plain_contrast())
            .set(POWER_LABEL, ui);
        
        // 48 bus
        
        Label::new(format!("48 Bus").as_str())
            .xy((-ui.win_w / 2.0) + 40.0, (ui.win_h / 2.0) - 220.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(BUS_48_LABEL, ui);
        
        Label::new(format!("{}V", 48).as_str())
            .xy((-ui.win_w / 2.0) + 40.0, (ui.win_h / 2.0) - 240.0)
            .font_size(16)
            .color(rgb(0.0, 1.0, 0.0))
            .set(V48_LABEL, ui);
        
        Label::new(format!("{}A", 15).as_str())
            .xy((-ui.win_w / 2.0) + 120.0, (ui.win_h / 2.0) - 240.0)
            .font_size(16)
            .color(rgb(0.0, 1.0, 0.0))
            .set(A48_LABEL, ui);
        
        // 24 bus
        
        Label::new(format!("24 H-Bus").as_str())
            .xy((-ui.win_w / 2.0) + 40.0, (ui.win_h / 2.0) - 280.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(BUS_24_LABEL, ui);
        
        Label::new(format!("{}V", 24.5).as_str())
            .xy((-ui.win_w / 2.0) + 40.0, (ui.win_h / 2.0) - 300.0)
            .font_size(16)
            .color(rgb(0.0, 1.0, 0.0))
            .set(V24_LABEL, ui);
        
        Label::new(format!("{}A", 3.5).as_str())
            .xy((-ui.win_w / 2.0) + 120.0, (ui.win_h / 2.0) - 300.0)
            .font_size(16)
            .color(rgb(0.0, 1.0, 0.0))
            .set(A24_LABEL, ui);
        
        // 12 bus
        
        Label::new(format!("P-12 Bus").as_str())
            .xy((-ui.win_w / 2.0) + 40.0, (ui.win_h / 2.0) - 340.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(BUS_12_LABEL, ui);
        
        Label::new(self.volts_12.as_str())
            .xy((-ui.win_w / 2.0) + 40.0, (ui.win_h / 2.0) - 360.0)
            .font_size(16)
            .color(rgb(0.0, 1.0, 0.0))
            .set(V12_LABEL, ui);
        
        Label::new(format!("{}A", 1.3).as_str())
            .xy((-ui.win_w / 2.0) + 120.0, (ui.win_h / 2.0) - 360.0)
            .font_size(16)
            .color(rgb(0.0, 1.0, 0.0))
            .set(A12_LABEL, ui);

        // Draw our UI!
        ui.draw(c, gl);
        
        self.volt_graph_12.draw(c.trans(ui.win_w - 205.0, 100.0), gl, &mut *ui.glyph_cache.borrow_mut());
    }
    
    pub fn handle_packet(&mut self, packet: String) {
        //println!("Got packet: {}", packet);
        let packet_parts: Vec<String> = packet.split(":").map(|s| s.to_string()).collect();
        
        match packet_parts[0].as_str() {
            "RPM_STATUS" => {
                self.l_rpm_status = packet_parts[1].clone();
                self.r_rpm_status = packet_parts[2].clone();
            },
            "12V_VOLTAGE" => {
                let point_x = self.volt_graph_12.num_points() as f64;
                self.volt_graph_12.add_point(point_x, packet_parts[1].parse().unwrap());
                if self.volt_graph_12.num_points() > 100 {
                    self.volt_graph_12.x_interval = ((self.volt_graph_12.num_points() - 100) as f64,
                                                      self.volt_graph_12.num_points() as f64);
                }
                self.volts_12 = format!("{}V", packet_parts[1]);
            },
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
}

// Widget IDs
const LOCAL_TIME: WidgetId = 0;
const UTC_TIME: WidgetId = LOCAL_TIME + 1;
const MISSION_TIME_LABEL: WidgetId = UTC_TIME + 1;
const MISSION_START_BUTTON: WidgetId = MISSION_TIME_LABEL + 1;
const MISSION_RESET_BUTTON: WidgetId = MISSION_START_BUTTON + 1;
const TIME_DELAY: WidgetId = MISSION_RESET_BUTTON + 1;

// Power section
const POWER_LABEL: WidgetId = TIME_DELAY + 1;

const BUS_48_LABEL: WidgetId = POWER_LABEL + 1;
const V48_LABEL: WidgetId = BUS_48_LABEL + 1;
const A48_LABEL: WidgetId = V48_LABEL + 1;

const BUS_24_LABEL: WidgetId = A48_LABEL + 1;
const V24_LABEL: WidgetId = BUS_24_LABEL + 1;
const A24_LABEL: WidgetId = V24_LABEL + 1;

const BUS_12_LABEL: WidgetId = A24_LABEL + 1;
const V12_LABEL: WidgetId = BUS_12_LABEL + 1;
const A12_LABEL: WidgetId = V12_LABEL + 1;