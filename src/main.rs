//! 
//!
//! A demonstration of all widgets available in Conrod.
//!
//!
//! Don't be put off by the number of method calls, they are only for demonstration and almost all
//! of them are optional. Conrod supports `Theme`s, so if you don't give it an argument, it will
//! check the current `Theme` within the `Ui` and retrieve defaults from there.
//!
//!

use std::net::UdpSocket;

extern crate piston;
extern crate conrod;
extern crate graphics;
extern crate opengl_graphics;
extern crate glutin_window;
extern crate vecmath;

use conrod::{
    Background,
    Button,
    Color,
    Colorable,
    Frameable,
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
use opengl_graphics::{GlGraphics, OpenGL};
use opengl_graphics::glyph_cache::GlyphCache;
use piston::event::*;
use piston::window::{WindowSettings, Size};
use glutin_window::GlutinWindow;
use std::path::Path;

struct RoverUi {
    bg_color: Color,
    l_rpm: f32,
    r_rpm: f32,
    
    socket: UdpSocket,
}

impl RoverUi {
    fn new(socket: UdpSocket) -> RoverUi {
        RoverUi {
            bg_color: rgb(0.2, 0.35, 0.45),
            l_rpm: 0.0,
            r_rpm: 0.0,
            socket: socket,
        }
    }
}

fn main() {
    let opengl = OpenGL::_3_2;
    let window = GlutinWindow::new(
        opengl,
        WindowSettings::new(
            "PISCES Rover Controller".to_string(),
            Size { width: 1100, height: 550 }
        )
        .exit_on_esc(true)
        .samples(4)
    );
    let event_iter = window.events().ups(60).max_fps(60);
    let mut gl = GlGraphics::new(opengl);

    let font_path = Path::new("./assets/fonts/NotoSans-Regular.ttf");
    let theme = Theme::default();
    let glyph_cache = GlyphCache::new(&font_path).unwrap();
    let mut ui = Ui::new(glyph_cache, theme);
    
    // Create a UDP socket to talk to the rover
    let socket = UdpSocket::bind("0.0.0.0:30001").unwrap();
    
    let mut rover_ui = RoverUi::new(socket);

    for event in event_iter {
        ui.handle_event(&event);
        if let Some(args) = event.render_args() {
            gl.draw(args.viewport(), |_, gl| {
                draw_ui(gl, &mut ui, &mut rover_ui);
            });
        }
    }
}



/// Draw the User Interface.
fn draw_ui<'a>(gl: &mut GlGraphics, ui: &mut Ui<GlyphCache<'a>>, rover_ui: &mut RoverUi) {
    // Draw the background.
    Background::new().color(rover_ui.bg_color).draw(ui, gl);

    // Left RPM slider
    Slider::new(rover_ui.l_rpm, -1000.0, 1000.0)
        .dimensions(200.0, 30.0)
        .xy(110.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 25.0)
        .rgb(0.5, 0.3, 0.6)
        .frame(1.0)
        .label("Left RPM")
        .label_color(white())
        .react(|new_rpm| {
            rover_ui.l_rpm = new_rpm;
            
            let rpm_packet = format!("{}:{}", rover_ui.l_rpm as i32, rover_ui.r_rpm as i32);
            rover_ui.socket.send_to(rpm_packet.as_bytes(), ("192.168.240.1", 30001)).unwrap();
        })
        .set(L_RPM_SLIDER, ui);
    
    // Right RPM slider
    Slider::new(rover_ui.r_rpm, -1000.0, 1000.0)
        .dimensions(200.0, 30.0)
        .xy((ui.win_w / 2.0) - 110.0, (ui.win_h / 2.0) - 25.0)
        .rgb(0.5, 0.3, 0.6)
        .frame(1.0)
        .label("Right RPM")
        .label_color(white())
        .react(|new_rpm| {
            rover_ui.r_rpm = new_rpm;
            
            let rpm_packet = format!("{}:{}", rover_ui.l_rpm as i32, rover_ui.r_rpm as i32);
            rover_ui.socket.send_to(rpm_packet.as_bytes(), ("192.168.240.1", 30001)).unwrap();
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
            
            let rpm_packet = format!("{}:{}", rover_ui.l_rpm as i32, rover_ui.r_rpm as i32);
            rover_ui.socket.send_to(rpm_packet.as_bytes(), ("192.168.240.1", 30001)).unwrap();
        })
        .set(STOP_BUTTON, ui);

    // Draw our Ui!
    ui.draw(gl);
    
    // Do some networking
    //let rpm_packet = format!("{}:{}", rover_ui.l_rpm as i32, rover_ui.r_rpm as i32);
    //rover_ui.socket.send_to(rpm_packet.as_bytes(), ("192.168.240.1", 30001)).unwrap();
}

// Widget IDs
const TITLE: WidgetId = 0;
const L_RPM_SLIDER: WidgetId = TITLE + 1;
const R_RPM_SLIDER: WidgetId = L_RPM_SLIDER + 1;
const STOP_BUTTON: WidgetId = R_RPM_SLIDER + 1;