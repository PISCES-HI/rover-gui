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
    
    v12_graph: LineGraph,
    va_12: Option<(f64, f64)>,
    
    // Motor temp
    motor_temp_graph: LineGraph,
    l_motor_temp: Option<f64>,
    r_motor_temp: Option<f64>,
}

impl TelemetryUi {
    pub fn new(socket: UdpSocket) -> TelemetryUi {
        let v12_graph = LineGraph::new((400.0, 150.0), (0.0, 100.0), (0.0, 20.0));
        let motor_temp_graph = LineGraph::new((400.0, 150.0), (0.0, 100.0), (0.0, 100.0));
    
        TelemetryUi {
            socket: socket,
        
            bg_color: rgb(0.2, 0.35, 0.45),
            
            mission_time: MissionTime::Paused(time::Duration::zero()),
            
            l_rpm_status: "UNAVAILABLE".to_string(),
            r_rpm_status: "UNAVAILABLE".to_string(),
            
            v12_graph: v12_graph,
            va_12: None,
            
            motor_temp_graph: motor_temp_graph,
            l_motor_temp: None,
            r_motor_temp: None,
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
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 220.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(BUS_48_LABEL, ui);
        
        Label::new(format!("{}V", 48).as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 240.0)
            .font_size(16)
            .color(rgb(0.0, 1.0, 0.0))
            .set(V48_LABEL, ui);
        
        Label::new(format!("{}A", 15).as_str())
            .xy((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 240.0)
            .font_size(16)
            .color(rgb(0.0, 1.0, 0.0))
            .set(A48_LABEL, ui);
        
        // 24 bus
        
        Label::new(format!("24 H-Bus").as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 280.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(BUS_24_LABEL, ui);
        
        Label::new(format!("{}V", 24.5).as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 300.0)
            .font_size(16)
            .color(rgb(0.0, 1.0, 0.0))
            .set(V24_LABEL, ui);
        
        Label::new(format!("{}A", 3.5).as_str())
            .xy((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 300.0)
            .font_size(16)
            .color(rgb(0.0, 1.0, 0.0))
            .set(A24_LABEL, ui);
        
        // 12 bus
        
        Label::new(format!("P-12 E Bus").as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 340.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(BUS_12_LABEL, ui);
        
        let (volts_12, amps_12, va_12_color) =
            match self.va_12 {
                Some((v, a)) => {
                    (format!("{0:.2}V", v),
                     format!("{0:.2}A", a),
                     rgb(0.0, 1.0, 0.0))
                },
                None => {
                    ("UNAVAILABLE".to_string(),
                     "UNAVAILABLE".to_string(),
                     rgb(1.0, 0.0, 0.0))
                },
            };
        Label::new(volts_12.as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 360.0)
            .font_size(16)
            .color(va_12_color)
            .set(V12_LABEL, ui);
        
        Label::new(amps_12.as_str())
            .xy((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 360.0)
            .font_size(16)
            .color(va_12_color)
            .set(A12_LABEL, ui);
            
        // Left motor
        
        Label::new(format!("L Motor").as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 400.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(L_MOTOR_POWER_LABEL, ui);
        
        Label::new("75% RPM")
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 420.0)
            .font_size(16)
            .color(rgb(0.0, 1.0, 0.0))
            .set(L_MOTOR_RPM_LABEL, ui);
        
        Label::new("2.5A")
            .xy((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 420.0)
            .font_size(16)
            .color(rgb(0.0, 1.0, 0.0))
            .set(L_MOTOR_AMP_LABEL, ui);
        
        // Right motor
        
        Label::new(format!("R Motor").as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 460.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(R_MOTOR_POWER_LABEL, ui);
        
        Label::new("75% RPM")
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 480.0)
            .font_size(16)
            .color(rgb(0.0, 1.0, 0.0))
            .set(R_MOTOR_RPM_LABEL, ui);
        
        Label::new("2.6A")
            .xy((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 480.0)
            .font_size(16)
            .color(rgb(0.0, 1.0, 0.0))
            .set(R_MOTOR_AMP_LABEL, ui);
            
        ////////////////////////////////////////////////////////////////////////////////////////////
        // Temp section
        
        Label::new("Temp")
            .xy((-ui.win_w / 2.0) + 410.0, (ui.win_h / 2.0) - 190.0)
            .font_size(20)
            .color(self.bg_color.plain_contrast())
            .set(TEMP_LABEL, ui);
        
        // Left motor temp
        
        Label::new(format!("L Motor").as_str())
            .xy((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 220.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(L_MOTOR_TEMP_LABEL, ui);
        
        let (l_motor_temp, l_motor_temp_color) =
            match self.l_motor_temp {
                Some(temp) => {
                    (format!("{0:.2} C", temp), rgb(0.0, 1.0, 0.0))
                },
                None => ("UNAVAILABLE".to_string(), rgb(1.0, 0.0, 0.0)),
            };
        Label::new(l_motor_temp.as_str())
            .xy((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 220.0)
            .font_size(16)
            .color(l_motor_temp_color)
            .set(L_MOTOR_C_LABEL, ui);
        
        // Right motor temp
        
        Label::new(format!("R Motor").as_str())
            .xy((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 240.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(R_MOTOR_TEMP_LABEL, ui);
        
        let (r_motor_temp, r_motor_temp_color) =
            match self.r_motor_temp {
                Some(temp) => {
                    (format!("{0:.2} C", temp), rgb(0.0, 1.0, 0.0))
                },
                None => ("UNAVAILABLE".to_string(), rgb(1.0, 0.0, 0.0)),
            };
        Label::new(r_motor_temp.as_str())
            .xy((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 240.0)
            .font_size(16)
            .color(r_motor_temp_color)
            .set(R_MOTOR_C_LABEL, ui);

        // Draw our UI!
        ui.draw(c, gl);
        
        self.v12_graph.draw(c.trans(ui.win_w - 405.0, 100.0), gl, &mut *ui.glyph_cache.borrow_mut());
        self.motor_temp_graph.draw(c.trans(ui.win_w - 405.0, 320.0), gl, &mut *ui.glyph_cache.borrow_mut());
    }
    
    pub fn handle_packet(&mut self, packet: String) {
        //println!("Got packet: {}", packet);
        let packet_parts: Vec<String> = packet.split(":").map(|s| s.to_string()).collect();
        
        match packet_parts[0].as_str() {
            "RPM_STATUS" => {
                self.l_rpm_status = packet_parts[1].clone();
                self.r_rpm_status = packet_parts[2].clone();
            },
            "P-12E" => {
                let point_x = self.v12_graph.num_points() as f64;
                let volts_12 = packet_parts[1].parse().unwrap();
                let amps_12 = packet_parts[2].parse().unwrap();
                self.v12_graph.add_point(point_x, volts_12);
                if self.v12_graph.num_points() > 100 {
                    self.v12_graph.x_interval = ((self.v12_graph.num_points() - 100) as f64,
                                                      self.v12_graph.num_points() as f64);
                }
                self.va_12 = Some((volts_12, amps_12));
            },
            "L_MOTOR_TEMP" => {
                let point_x = self.motor_temp_graph.num_points() as f64;
                let l_motor_temp = packet_parts[1].parse().unwrap();
                self.motor_temp_graph.add_point(point_x, l_motor_temp);
                if self.motor_temp_graph.num_points() > 100 {
                    self.motor_temp_graph.x_interval = ((self.motor_temp_graph.num_points() - 100) as f64,
                                                      self.motor_temp_graph.num_points() as f64);
                }
                self.l_motor_temp = Some(l_motor_temp);
            },
            "R_MOTOR_TEMP" => {
                //let point_x = self.motor_temp_graph.num_points() as f64;
                let r_motor_temp = packet_parts[1].parse().unwrap();
                /*self.motor_temp_graph.add_point(point_x, l_motor_temp);
                if self.motor_temp_graph.num_points() > 100 {
                    self.motor_temp_graph.x_interval = ((self.motor_temp_graph.num_points() - 100) as f64,
                                                      self.motor_temp_graph.num_points() as f64);
                }*/
                self.r_motor_temp = Some(r_motor_temp);
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

const L_MOTOR_POWER_LABEL: WidgetId = A12_LABEL + 1;
const L_MOTOR_RPM_LABEL: WidgetId = L_MOTOR_POWER_LABEL + 1;
const L_MOTOR_AMP_LABEL: WidgetId = L_MOTOR_RPM_LABEL + 1;

const R_MOTOR_POWER_LABEL: WidgetId = L_MOTOR_AMP_LABEL + 1;
const R_MOTOR_RPM_LABEL: WidgetId = R_MOTOR_POWER_LABEL + 1;
const R_MOTOR_AMP_LABEL: WidgetId = R_MOTOR_RPM_LABEL + 1;

// Temp section
const TEMP_LABEL: WidgetId = R_MOTOR_AMP_LABEL + 1;

const L_MOTOR_TEMP_LABEL: WidgetId = TEMP_LABEL + 1;
const L_MOTOR_C_LABEL: WidgetId = L_MOTOR_TEMP_LABEL + 1;

const R_MOTOR_TEMP_LABEL: WidgetId = L_MOTOR_C_LABEL + 1;
const R_MOTOR_C_LABEL: WidgetId = R_MOTOR_TEMP_LABEL + 1;