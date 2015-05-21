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
}

impl RoverUi {
    fn new() -> RoverUi {
        RoverUi {
            bg_color: rgb(0.2, 0.35, 0.45),
            l_rpm: 0.0,
            r_rpm: 0.0,
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
    let mut demo = RoverUi::new();
    
    // Create a UDP socket to talk to the rover
    /*let socket = UdpSocket::bind("0.0.0.0:30001").unwrap();
    
    let mut buf = [0; 10];
    let (bytes_read, src) = socket.recv_from(&mut buf).unwrap();
    
    // Send a reply to the socket we received data from
    let buf = &mut buf[..bytes_read];
    buf.reverse();
    socket.send_to(buf, &src).unwrap();*/

    for event in event_iter {
        ui.handle_event(&event);
        if let Some(args) = event.render_args() {
            gl.draw(args.viewport(), |_, gl| {
                draw_ui(gl, &mut ui, &mut demo);
            });
        }
    }
}



/// Draw the User Interface.
fn draw_ui<'a>(gl: &mut GlGraphics, ui: &mut Ui<GlyphCache<'a>>, demo: &mut RoverUi) {

    // Draw the background.
    Background::new().color(demo.bg_color).draw(ui, gl);

    // Left RPM slider
    Slider::new(demo.l_rpm, 0.0, 100.0)
        .dimensions(200.0, 30.0)
        .xy(110.0 - (ui.win_w / 2.0), (ui.win_h / 2.0) - 25.0)
        .rgb(0.5, 0.3, 0.6)
        .frame(1.0)
        .label("Left RPM")
        .label_color(white())
        .react(|new_rpm| demo.l_rpm = new_rpm)
        .set(L_RPM_SLIDER, ui);
    
    // Right RPM slider
    Slider::new(demo.r_rpm, 0.0, 100.0)
        .dimensions(200.0, 30.0)
        .xy((ui.win_w / 2.0) - 110.0, (ui.win_h / 2.0) - 25.0)
        .rgb(0.5, 0.3, 0.6)
        .frame(1.0)
        .label("Right RPM")
        .label_color(white())
        .react(|new_rpm| demo.r_rpm = new_rpm)
        .set(R_RPM_SLIDER, ui);

    // Draw our Ui!
    ui.draw(gl);
}

// Widget IDs
const TITLE: WidgetId = 0;
const L_RPM_SLIDER: WidgetId = TITLE + 1;
const R_RPM_SLIDER: WidgetId = L_RPM_SLIDER + 1;