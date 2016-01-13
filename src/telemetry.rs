#![feature(iter_arith)]
#![feature(convert)]

use std::cell::RefCell;
use std::fs;
use std::net::UdpSocket;
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::channel;
use std::thread;

#[macro_use]
extern crate conrod;
extern crate time;
extern crate piston;
extern crate graphics;
extern crate opengl_graphics;
extern crate sdl2_window;

use conrod::{
    Theme,
    Ui,
};
use opengl_graphics::{GlGraphics, OpenGL, Texture};
use opengl_graphics::glyph_cache::GlyphCache;
use piston::input;
use piston::input::*;
use piston::event_loop::*;
use piston::window::{WindowSettings, Size};
use sdl2_window::Sdl2Window;

use tele_ui::TelemetryUi;

pub mod avg_val;
pub mod line_graph;
pub mod tele_ui;

fn main() {
    let opengl = OpenGL::V3_2;
    let window = Sdl2Window::new(
        WindowSettings::new(
            "PISCES Telemetry".to_string(),
            Size { width: 1280, height: 700 }
        )
        .exit_on_esc(true)
        .samples(4)
    ).unwrap();
    let window = Rc::new(RefCell::new(window));
    let mut gl = GlGraphics::new(opengl);

    let font_path = Path::new("./assets/fonts/NotoSans-Regular.ttf");
    let theme = Theme::default();
    let glyph_cache = GlyphCache::new(&font_path).unwrap();
    let mut ui = Ui::new(glyph_cache, theme);
    
    // Create a UDP socket to talk to the rover
    let socket = UdpSocket::bind("0.0.0.0:30001").ok().expect("Failed to open UDP socket");
    socket.send_to(b"connect me plz", ("10.10.155.165", 30001));
    
    let in_socket = socket.try_clone().unwrap();
    let (packet_t, packet_r) = channel();
    
    thread::Builder::new()
        .name("packet_in".to_string())
        .spawn(move || {
            let mut buf = [0u8; 512];
            loop {
                let (bytes_read, _) = in_socket.recv_from(&mut buf).unwrap();
                if let Ok(msg) = String::from_utf8(buf[0..bytes_read].iter().cloned().collect()) {
                    packet_t.send(msg).unwrap();
                }
            }
        }).unwrap();
    
    let mission_folder = format!("{}", time::now().strftime("%Y%b%d_%H_%M").unwrap());
    fs::create_dir_all(format!("mission_data/{}", mission_folder).as_str());
    let mut tele_ui = TelemetryUi::new(socket, mission_folder.as_str());
    
    ///////////////////////////////////////////////////////////////////////////////////////

    let mut last_update_time = time::now();

    let event_iter = window.clone().events().ups(20).max_fps(60);
    for e in event_iter {
        ui.handle_event(&e);
        
        e.press(|button| {
            match button {
                input::Button::Keyboard(key) => tele_ui.on_key_pressed(key), 
                _ => { },
            }
        });
        
        e.release(|button| {
            match button {
                input::Button::Keyboard(key) => tele_ui.on_key_released(key), 
                _ => { },
            }
        });
        
        // Update
        e.update(|_| {
            while let Ok(packet) = packet_r.try_recv() {
                tele_ui.handle_packet(packet);
            }

            // Log some data
            if (time::now()-last_update_time).num_seconds() >= 1 {
                last_update_time = time::now();
                tele_ui.log_data();
            }
        });
        
        // Render GUI
        e.render(|args| {
            gl.draw(args.viewport(), |c, gl| {
                tele_ui.draw_ui(c, gl, &mut ui);
            });
        });
    }
}
