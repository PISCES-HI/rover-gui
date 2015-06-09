#![feature(convert)]

use std::net::UdpSocket;
use std::sync::mpsc::channel;
use std::thread;

use sdl2::controller;

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

struct RoverUi {
    bg_color: Color,
    
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
    
    socket: UdpSocket,
}

impl RoverUi {
    fn new(socket: UdpSocket) -> RoverUi {
        RoverUi {
            bg_color: rgb(0.2, 0.35, 0.45),
            
            l_rpm: 0.0,
            r_rpm: 0.0,
            both_rpm: false,
            max_rpm: 2000.0,
            l_rpm_status: "UNAVAILABLE".to_string(),
            r_rpm_status: "UNAVAILABLE".to_string(),
            
            f_pan: 90.0,
            f_tilt: 130.0,
            
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
                let msg = String::from_utf8(buf[0..bytes_read].iter().cloned().collect()).unwrap();
                packet_t.send(msg).unwrap();
            }
        }).unwrap();
    
    let mut rover_ui = RoverUi::new(socket);
    rover_ui.send_rpm();
    rover_ui.send_f_pan();
    rover_ui.send_f_tilt();

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
                let rpm_parts: Vec<String> = msg.split(":").map(|s| s.to_string()).collect();
                rover_ui.l_rpm_status = rpm_parts[0].clone();
                rover_ui.r_rpm_status = rpm_parts[1].clone();
            }
            
            if let Some(ref controller) = controller {
                // Control RPM with analog sticks
                let left_y = controller.get_axis(controller::Axis::LeftY).unwrap();
                let right_y = controller.get_axis(controller::Axis::RightY).unwrap();
                
                let l_rpm = -(left_y as f32 / 32768.0) * rover_ui.max_rpm;
                let r_rpm = -(right_y as f32 / 32768.0) * rover_ui.max_rpm;
                
                rover_ui.try_update_rpm(l_rpm, r_rpm);
                
                // Control pan with left/right arrow keys
                if controller.get_button(controller::Button::DPadLeft).unwrap() {
                    rover_ui.f_pan -= f32::min(5.0, rover_ui.f_pan - 0.0);
                    rover_ui.send_f_pan();
                }
                if controller.get_button(controller::Button::DPadRight).unwrap() {
                    rover_ui.f_pan += f32::min(5.0, 180.0 - rover_ui.f_pan);
                    rover_ui.send_f_pan();
                }
                
                // Control tilt with up/down arrow keys
                if controller.get_button(controller::Button::DPadDown).unwrap() {
                    rover_ui.f_tilt -= f32::min(5.0, rover_ui.f_tilt - 90.0);
                    rover_ui.send_f_tilt();
                }
                if controller.get_button(controller::Button::DPadUp).unwrap() {
                    rover_ui.f_tilt += f32::min(5.0, 180.0 - rover_ui.f_tilt);
                    rover_ui.send_f_tilt();
                }
            }
        });
        
        // Render GUI
        e.render(|args| {
            gl.draw(args.viewport(), |c, gl| {
                draw_ui(c, gl, &mut ui, &mut rover_ui);
            });
        });
    }
}

/// Draw the User Interface.
fn draw_ui<'a>(c: Context, gl: &mut GlGraphics, ui: &mut Ui<GlyphCache<'a>>, rover_ui: &mut RoverUi) {
    // Draw the background.
    Background::new().color(rover_ui.bg_color).draw(ui, gl);

    // Left RPM slider
    let l_rpm =
        if rover_ui.both_rpm {
            rover_ui.l_rpm.max(rover_ui.r_rpm)
        } else {
            rover_ui.l_rpm
        };
    Slider::new(l_rpm, -rover_ui.max_rpm, rover_ui.max_rpm)
        .dimensions(200.0, 30.0)
        .xy(110.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 25.0)
        .rgb(0.5, 0.3, 0.6)
        .frame(1.0)
        .label("Left RPM")
        .label_color(white())
        .react(|new_rpm| {
            if !rover_ui.both_rpm {
                rover_ui.try_update_l_rpm(new_rpm);
            } else {
                rover_ui.try_update_rpm(new_rpm, new_rpm);
            }
        })
        .set(L_RPM_SLIDER, ui);
    
    // Right RPM slider
    let r_rpm =
        if rover_ui.both_rpm {
            rover_ui.l_rpm.max(rover_ui.r_rpm)
        } else {
            rover_ui.r_rpm
        };
    Slider::new(r_rpm, -rover_ui.max_rpm, rover_ui.max_rpm)
        .dimensions(200.0, 30.0)
        .xy((ui.win_w / 2.0) - 110.0, (ui.win_h / 2.0) - 25.0)
        .rgb(0.5, 0.3, 0.6)
        .frame(1.0)
        .label("Right RPM")
        .label_color(white())
        .react(|new_rpm| {
            if !rover_ui.both_rpm {
                rover_ui.try_update_r_rpm(new_rpm);
            } else {
                rover_ui.try_update_rpm(new_rpm, new_rpm);
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
            rover_ui.l_rpm = 0.0;
            rover_ui.r_rpm = 0.0;
            rover_ui.send_rpm();
        })
        .set(STOP_BUTTON, ui);
    
    // Left status RPM
    Label::new(rover_ui.l_rpm_status.as_str())
        .xy(110.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 60.0)
        .font_size(32)
        .color(rover_ui.bg_color.plain_contrast())
        .set(L_RPM_STATUS, ui);
    
    // Right status RPM
    Label::new(rover_ui.r_rpm_status.as_str())
        .xy((ui.win_w / 2.0) - 110.0, (ui.win_h / 2.0) - 60.0)
        .font_size(32)
        .color(rover_ui.bg_color.plain_contrast())
        .set(R_RPM_STATUS, ui);
    
    // Camera pan slider
    Slider::new(rover_ui.f_pan, 0.0, 180.0)
        .dimensions(200.0, 30.0)
        .xy(110.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 110.0)
        .rgb(0.5, 0.3, 0.6)
        .frame(1.0)
        .label("Forward Pan")
        .label_color(white())
        .react(|new_pan| {
            rover_ui.try_update_f_pan(new_pan);
        })
        .set(F_PAN_SLIDER, ui);
    
    // Camera tilt slider
    Slider::new(rover_ui.f_tilt, 90.0, 180.0)
        .dimensions(200.0, 30.0)
        .xy(110.0 - (ui.win_w / 2.0) + 210.0, (ui.win_h / 2.0) - 110.0)
        .rgb(0.5, 0.3, 0.6)
        .frame(1.0)
        .label("Forward Tilt")
        .label_color(white())
        .react(|new_tilt| {
            rover_ui.try_update_f_tilt(new_tilt);
        })
        .set(F_TILT_SLIDER, ui);

    // Draw our UI!
    ui.draw(c, gl);
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