#![feature(convert)]

use std::net::UdpSocket;
use std::ops::DerefMut;
use std::sync::mpsc::channel;
use std::thread;

use sdl2::controller;

extern crate time;
extern crate sdl2;
extern crate piston;
extern crate conrod;
extern crate graphics;
extern crate opengl_graphics;
extern crate sdl2_window;

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
    Theme,
    Ui,
    WidgetId,
    Widget,
};
use conrod::color::{rgb, white};
use graphics::Context;
use opengl_graphics::{GlGraphics, OpenGL};
use opengl_graphics::glyph_cache::GlyphCache;
use piston::input;
use piston::event::*;
use piston::window::{WindowSettings, Size};
use sdl2_window::Sdl2Window;
use std::path::Path;

use line_graph::LineGraph;

pub mod line_graph;

struct RoverUi {
    bg_color: Color,
    
    start_time: time::Tm,
    
    // RPM stuff
    l_rpm: f32,
    r_rpm: f32,
    both_rpm: bool,
    max_rpm: f32,
    l_rpm_status: String,
    r_rpm_status: String,
    
    // Forward camera controls
    f_pan: f32,
    f_tilt: f32,
    
    // Blade controls
    blade: f32,
    
    voltage_graph: LineGraph,
    
    socket: UdpSocket,
}

impl RoverUi {
    fn new(socket: UdpSocket) -> RoverUi {
        let voltage_graph = LineGraph::new((200.0, 100.0), (0.0, 100.0), (0.0, 20.0));
    
        RoverUi {
            bg_color: rgb(0.2, 0.35, 0.45),
            
            start_time: time::now(),
            
            l_rpm: 0.0,
            r_rpm: 0.0,
            both_rpm: false,
            max_rpm: 2000.0,
            l_rpm_status: "UNAVAILABLE".to_string(),
            r_rpm_status: "UNAVAILABLE".to_string(),
            
            f_pan: 90.0,
            f_tilt: 130.0,
            
            blade: 0.0,
            
            voltage_graph: voltage_graph,
            
            socket: socket,
        }
    }
    
    fn on_key_pressed(&mut self, key: input::Key) {
        match key {
            input::Key::RCtrl | input::Key::LCtrl => {
                self.both_rpm = true;
            },
            _ => { },
        }
    }
    
    fn on_key_released(&mut self, key: input::Key) {
        match key {
            input::Key::RCtrl | input::Key::LCtrl => {
                self.both_rpm = false;
            },
            _ => { },
        }
    }
    
    pub fn try_update_rpm(&mut self, l_rpm: f32, r_rpm: f32) {
        if (l_rpm - self.l_rpm).abs() > 5.0 || (r_rpm - self.r_rpm).abs() > 5.0 {
            self.l_rpm = l_rpm;
            self.r_rpm = r_rpm;
            self.send_rpm();
        }
    }
    
    pub fn try_update_l_rpm(&mut self, l_rpm: f32) {
        if (l_rpm - self.l_rpm).abs() > 5.0 {
            self.l_rpm = l_rpm;
            self.send_rpm();
        }
    }
    
    pub fn try_update_r_rpm(&mut self, r_rpm: f32) {
        if (r_rpm - self.r_rpm).abs() > 5.0 {
            self.r_rpm = r_rpm;
            self.send_rpm();
        }
    }
    
    pub fn try_update_f_pan(&mut self, f_pan: f32) {
        if (f_pan - self.f_pan).abs() > 5.0 || f_pan == 0.0 || f_pan == 180.0 {
            self.f_pan = f_pan;
            self.send_f_pan();
        }
    }
    
    pub fn try_update_f_tilt(&mut self, f_tilt: f32) {
        if (f_tilt - self.f_tilt).abs() > 5.0 || f_tilt == 90.0 || f_tilt == 180.0 {
            self.f_tilt = f_tilt;
            self.send_f_tilt();
        }
    }
    
    pub fn try_update_blade(&mut self, blade: f32) {
        if (blade - self.blade).abs() > 1.0 || blade == -10.0 || blade == 10.0 {
            self.blade = blade;
            self.send_blade();
        }
    }
    
    fn send_rpm(&self) {
        let packet = format!("A{}:{}", self.l_rpm as i32, self.r_rpm as i32);
        self.socket.send_to(packet.as_bytes(), ("10.10.153.25", 30001)).unwrap();
    }
    
    fn send_f_pan(&self) {
        let packet = format!("B{}", self.f_pan as i32);
        self.socket.send_to(packet.as_bytes(), ("10.10.153.25", 30001)).unwrap();
    }
    
    fn send_f_tilt(&self) {
        let packet = format!("C{}", self.f_tilt as i32);
        self.socket.send_to(packet.as_bytes(), ("10.10.153.25", 30001)).unwrap();
    }
    
    fn send_blade(&self) {
        let packet = format!("D{}", self.blade as i32);
        self.socket.send_to(packet.as_bytes(), ("10.10.153.25", 30001)).unwrap();
    }
    
    fn draw_ui<'a>(&mut self, c: Context, gl: &mut GlGraphics, ui: &mut Ui<GlyphCache<'a>>) {
        use graphics::*;
    
        // Draw the background.
        Background::new().color(self.bg_color).draw(ui, gl);

        // Left RPM slider
        let l_rpm =
            if self.both_rpm {
                self.l_rpm.max(self.r_rpm)
            } else {
                self.l_rpm
            };
        Slider::new(l_rpm, -self.max_rpm, self.max_rpm)
            .dimensions(200.0, 30.0)
            .xy(110.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 25.0)
            .rgb(0.5, 0.3, 0.6)
            .frame(1.0)
            .label("Left RPM")
            .label_color(white())
            .react(|new_rpm| {
                if !self.both_rpm {
                    self.try_update_l_rpm(new_rpm);
                } else {
                    self.try_update_rpm(new_rpm, new_rpm);
                }
            })
            .set(L_RPM_SLIDER, ui);
        
        // Right RPM slider
        let r_rpm =
            if self.both_rpm {
                self.l_rpm.max(self.r_rpm)
            } else {
                self.r_rpm
            };
        Slider::new(r_rpm, -self.max_rpm, self.max_rpm)
            .dimensions(200.0, 30.0)
            .xy((ui.win_w / 2.0) - 110.0, (ui.win_h / 2.0) - 25.0)
            .rgb(0.5, 0.3, 0.6)
            .frame(1.0)
            .label("Right RPM")
            .label_color(white())
            .react(|new_rpm| {
                if !self.both_rpm {
                    self.try_update_r_rpm(new_rpm);
                } else {
                    self.try_update_rpm(new_rpm, new_rpm);
                }
            })
            .set(R_RPM_SLIDER, ui);
        
        // Stop button
        Button::new()
            .dimensions(200.0, 30.0)
            .xy(0.0, (ui.win_h / 2.0) - 25.0)
            .rgb(1.0, 0.0, 0.0)
            .frame(1.0)
            .label("Stop")
            .react(|| {
                self.l_rpm = 0.0;
                self.r_rpm = 0.0;
                self.send_rpm();
            })
            .set(STOP_BUTTON, ui);
        
        // Left status RPM
        Label::new(self.l_rpm_status.as_str())
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
        
        // Camera pan slider
        Slider::new(self.f_pan, 0.0, 180.0)
            .dimensions(200.0, 30.0)
            .xy(110.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 110.0)
            .rgb(0.5, 0.3, 0.6)
            .frame(1.0)
            .label("Forward Pan")
            .label_color(white())
            .react(|new_pan| {
                self.try_update_f_pan(new_pan);
            })
            .set(F_PAN_SLIDER, ui);
        
        // Camera tilt slider
        Slider::new(self.f_tilt, 90.0, 180.0)
            .dimensions(200.0, 30.0)
            .xy(110.0 - (ui.win_w / 2.0) + 210.0, (ui.win_h / 2.0) - 110.0)
            .rgb(0.5, 0.3, 0.6)
            .frame(1.0)
            .label("Forward Tilt")
            .label_color(white())
            .react(|new_tilt| {
                self.try_update_f_tilt(new_tilt);
            })
            .set(F_TILT_SLIDER, ui);
        
        // Camera tilt slider
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
        
        // Mission time label
        let mission_time = time::now() - self.start_time;
        let total_hours = mission_time.num_hours();
        let total_minutes = mission_time.num_minutes();
        let total_seconds = mission_time.num_seconds();
        let total_milliseconds = mission_time.num_milliseconds();
        
        let hours = total_hours;
        let minutes = total_minutes - total_hours*60;
        let seconds = total_seconds - total_minutes*60;
        let milliseconds = total_milliseconds - total_seconds*1000;
        Label::new(format!("Mission Time: {}:{}:{}:{}", hours, minutes, seconds, milliseconds).as_str())
            .xy(0.0, (-ui.win_h / 2.0) + 35.0)
            .font_size(32)
            .color(self.bg_color.plain_contrast())
            .set(MISSION_TIME_LABEL, ui);

        // Draw our UI!
        ui.draw(c, gl);
        
        // Draw telemetry graphs
        self.voltage_graph.draw(c.trans(5.0, 250.0), gl, ui.glyph_cache.borrow_mut().deref_mut());
    }
}

fn main() {
    let opengl = OpenGL::_3_2;
    let window = Sdl2Window::new(
        WindowSettings::new(
            "PISCES Rover Controller".to_string(),
            Size { width: 1100, height: 550 }
        )
        .exit_on_esc(true)
        .samples(4)
    );
    let event_iter = window.events().ups(20).max_fps(60);
    let mut gl = GlGraphics::new(opengl);

    let font_path = Path::new("./assets/fonts/NotoSans-Regular.ttf");
    let theme = Theme::default();
    let glyph_cache = GlyphCache::new(&font_path).unwrap();
    let mut ui = Ui::new(glyph_cache, theme);
    
    // Initialize game pad
    let controller = init_game_pad();
    
    // Create a UDP socket to talk to the rover
    let socket = UdpSocket::bind("0.0.0.0:30001").unwrap();
    
    let in_socket = socket.try_clone().unwrap();
    let (packet_t, packet_r) = channel();
    
    thread::Builder::new()
        .name("packet_in".to_string())
        .spawn(move || {
            let mut buf = [0u8; 64];
            loop {
                let (bytes_read, _) = in_socket.recv_from(&mut buf).unwrap();
                if let Ok(msg) = String::from_utf8(buf[0..bytes_read].iter().cloned().collect()) {
                    packet_t.send(msg).unwrap();
                }
            }
        }).unwrap();
    
    let mut rover_ui = RoverUi::new(socket);
    rover_ui.send_rpm();
    rover_ui.send_f_pan();
    rover_ui.send_f_tilt();
    
    ///////////////////////////////////////////////////////////////////////////////////////

    for e in event_iter {
        ui.handle_event(&e);
        
        e.press(|button| {
            match button {
                input::Button::Keyboard(key) => rover_ui.on_key_pressed(key), 
                _ => { },
            }
        });
        
        e.release(|button| {
            match button {
                input::Button::Keyboard(key) => rover_ui.on_key_released(key), 
                _ => { },
            }
        });
        
        // Update
        e.update(|_| {
            while let Ok(msg) = packet_r.try_recv() {
                //println!("Got packet: {}", msg);
                let packet_parts: Vec<String> = msg.split(":").map(|s| s.to_string()).collect();
                
                match packet_parts[0].as_str() {
                    "RPM_STATUS" => {
                        rover_ui.l_rpm_status = packet_parts[1].clone();
                        rover_ui.r_rpm_status = packet_parts[2].clone();
                    },
                    "12V_VOLTAGE" => {
                        let point_x = rover_ui.voltage_graph.num_points() as f64;
                        rover_ui.voltage_graph.add_point(point_x, packet_parts[1].parse().unwrap());
                        if rover_ui.voltage_graph.num_points() > 100 {
                            rover_ui.voltage_graph.x_interval = ((rover_ui.voltage_graph.num_points() - 100) as f64,
                                                                 rover_ui.voltage_graph.num_points() as f64);
                        }
                    },
                    _ => { println!("Unknown packet ID: {}", packet_parts[0]) },
                }
            }
            
            if let Some(ref controller) = controller {
                // Control RPM with analog sticks
                let left_y = controller.get_axis(controller::Axis::LeftY);
                let right_y = controller.get_axis(controller::Axis::RightY);
                
                let l_rpm = -(left_y as f32 / 32768.0) * rover_ui.max_rpm;
                let r_rpm = -(right_y as f32 / 32768.0) * rover_ui.max_rpm;
                
                rover_ui.try_update_rpm(l_rpm, r_rpm);
                
                // Control pan with left/right arrow keys
                if controller.get_button(controller::Button::DPadLeft) {
                    rover_ui.f_pan -= f32::min(5.0, rover_ui.f_pan - 0.0);
                    rover_ui.send_f_pan();
                }
                if controller.get_button(controller::Button::DPadRight) {
                    rover_ui.f_pan += f32::min(5.0, 180.0 - rover_ui.f_pan);
                    rover_ui.send_f_pan();
                }
                
                // Control tilt with up/down arrow keys
                if controller.get_button(controller::Button::DPadDown) {
                    rover_ui.f_tilt -= f32::min(5.0, rover_ui.f_tilt - 90.0);
                    rover_ui.send_f_tilt();
                }
                if controller.get_button(controller::Button::DPadUp) {
                    rover_ui.f_tilt += f32::min(5.0, 180.0 - rover_ui.f_tilt);
                    rover_ui.send_f_tilt();
                }
            }
        });
        
        // Render GUI
        e.render(|args| {
            gl.draw(args.viewport(), |c, gl| {
                rover_ui.draw_ui(c, gl, &mut ui);
            });
        });
    }
}

pub fn init_game_pad() -> Option<controller::GameController> {
    use sdl2::{joystick, controller};
    
    println!("Looking for game controller...");

    let available =
        match joystick::num_joysticks() {
            Ok(n)  => n,
            Err(e) => panic!("can't enumerate joysticks: {}", e),
        };

    println!("{} joysticks available", available);

    // Iterate over all available joysticks and look for game
    // controllers.
    for id in 0..available {
        if controller::is_game_controller(id) {
            println!("Attempting to open game controller {}", id);

            match controller::GameController::open(id) {
                Ok(c) => {
                    // We managed to find and open a game controller,
                    // exit the loop
                    println!("Success: opened \"{}\"", c.name());
                    return Some(c);
                },
                Err(e) => println!("Failed to open game controller: {:?}", e),
            }

        } else {
             println!("{} is not a game controller", id);
        }
    }

    None
}

// Widget IDs
const TITLE: WidgetId = 0;
const L_RPM_SLIDER: WidgetId = TITLE + 1;
const R_RPM_SLIDER: WidgetId = L_RPM_SLIDER + 1;
const STOP_BUTTON: WidgetId = R_RPM_SLIDER + 1;
const L_RPM_STATUS: WidgetId = STOP_BUTTON + 1;
const R_RPM_STATUS: WidgetId = L_RPM_STATUS + 1;
const F_PAN_SLIDER: WidgetId = R_RPM_STATUS + 1;
const F_TILT_SLIDER: WidgetId = F_PAN_SLIDER + 1;
const BLADE_SLIDER: WidgetId = F_TILT_SLIDER + 1;
const VOLTAGE_12_LABEL: WidgetId = BLADE_SLIDER + 1;
const MISSION_TIME_LABEL: WidgetId = VOLTAGE_12_LABEL + 1;