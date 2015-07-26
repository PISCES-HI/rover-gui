#![feature(convert)]
use std::net::UdpSocket;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::channel;
use std::thread;

extern crate time;
extern crate sdl2;
extern crate piston;
extern crate conrod;
extern crate graphics;
extern crate opengl_graphics;
extern crate sdl2_window;
extern crate ffmpeg;
extern crate image;

use conrod::{
    Theme,
    Ui,
};
use opengl_graphics::{GlGraphics, OpenGL, Texture};
use opengl_graphics::glyph_cache::GlyphCache;
use piston::input;
use piston::event::*;
use piston::window::{WindowSettings, Size};
use sdl2::controller;
use sdl2_window::Sdl2Window;

use blade_ui::BladeUi;
use video_stream::{init_ffmpeg, start_video_stream};

pub mod line_graph;
pub mod blade_ui;
pub mod video_stream;

fn main() {
    init_ffmpeg();

    let opengl = OpenGL::_3_2;
    let window = Sdl2Window::new(
        WindowSettings::new(
            "PISCES Navigation".to_string(),
            Size { width: 1280, height: 700 }
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
    let controller = init_game_controller();
    
    // Create a UDP socket to talk to the rover
    let socket = UdpSocket::bind("0.0.0.0:30003").unwrap();
    socket.send_to(b"connect me plz", ("10.14.120.25", 30001));
    
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
    
    let mut blade_ui = BladeUi::new(socket);
    
    ////////////////////////////////////////////////////////////////////////////////////////
    
    let (mut video_texture, video_image) = start_video_stream("rtsp://root:pisces@10.14.120.28/axis-media/media.amp");
    
    ///////////////////////////////////////////////////////////////////////////////////////

    for e in event_iter {
        ui.handle_event(&e);
        
        e.press(|button| {
            match button {
                input::Button::Keyboard(key) => blade_ui.on_key_pressed(key), 
                _ => { },
            }
        });
        
        e.release(|button| {
            match button {
                input::Button::Keyboard(key) => blade_ui.on_key_released(key), 
                _ => { },
            }
        });
        
        // Update
        e.update(|_| {
            while let Ok(packet) = packet_r.try_recv() {
                blade_ui.handle_packet(packet);
            }
            
            if let Some(ref controller) = controller {
                // Control RPM with analog sticks
                let left_y = controller.get_axis(controller::Axis::LeftY);
                let blade = -(left_y as f32 / 32768.0) * 100.0;

                blade_ui.try_update_blade(blade);
            }
            
            let video_image = video_image.lock().unwrap();
            video_texture.update(&*video_image);
        });
        
        // Render GUI
        e.render(|args| {
            gl.draw(args.viewport(), |c, gl| {
                use graphics::*;
            
                blade_ui.draw_ui(c, gl, &mut ui);
                
                Rectangle::new([0.0, 0.0, 0.4, 1.0])
                    .draw([1280.0 - 700.0 - 5.0, 5.0, 700.0, 400.0],
                          &c.draw_state, c.transform,
                          gl);
                image(&video_texture, c.trans(1280.0 - 700.0 - 5.0, 5.0).scale(700.0/512.0, 400.0/512.0).transform, gl);
            });
        });
    }
}

pub fn init_game_controller() -> Option<controller::GameController> {
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
