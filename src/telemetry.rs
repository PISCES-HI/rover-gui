#![feature(iter_arith)]

use std::fs;
use std::net::UdpSocket;
use std::path::Path;
use std::sync::mpsc::channel;
use std::thread;

#[macro_use]
extern crate conrod;
extern crate time;
extern crate graphics;
extern crate piston_window;

use conrod::{
    Theme,
};
use piston_window::{EventLoop, Glyphs, PistonWindow, WindowSettings};

use conrod_config::Ui;
use tele_ui::TelemetryUi;

pub mod avg_val;
pub mod conrod_config;
pub mod line_graph;
pub mod tele_ui;

fn main() {
    let mut window: PistonWindow = WindowSettings::new("PISCES Telemetry".to_string(),
                                                       [1280, 700]).exit_on_esc(true)
                                                                   .build().unwrap();

    let mut ui = {
        let font_path = Path::new("./assets/fonts/NotoSans-Regular.ttf");
        let theme = Theme::default();
        let glyph_cache = Glyphs::new(&font_path, window.factory.clone());
        Ui::new(glyph_cache.unwrap(), theme)
    };
    
    // Create a UDP socket to talk to the rover
    let socket = UdpSocket::bind("0.0.0.0:30001").ok().expect("Failed to open UDP socket");
    socket.send_to(b"connect me plz", ("10.10.153.8", 30001)).unwrap();
    
    let in_socket = socket;
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
    fs::create_dir_all(format!("mission_data/{}", mission_folder).as_str()).unwrap();
    let mut tele_ui = TelemetryUi::new(mission_folder.as_str());
    
    ///////////////////////////////////////////////////////////////////////////////////////

    let mut last_update_time = time::now();

    window.set_ups(20);
    window.set_max_fps(60);

    while let Some(e) = window.next() {
        use piston_window::{Button, PressEvent, ReleaseEvent, UpdateEvent};

        ui.handle_event(&e);
        
        e.press(|button| {
            match button {
                Button::Keyboard(key) => tele_ui.on_key_pressed(key), 
                _ => { },
            }
        });

        e.release(|button| {
            match button {
                Button::Keyboard(key) => tele_ui.on_key_released(key), 
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
        window.draw_2d(&e, |c, g| {
            tele_ui.draw_ui(c, g, &mut ui);
        });
    }
}
