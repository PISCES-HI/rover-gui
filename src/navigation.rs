#![feature(convert)]
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
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
use sdl2_window::Sdl2Window;
use std::path::Path;

use ffmpeg::codec;
use ffmpeg::format;
use ffmpeg::media;
use ffmpeg::software::scaling;
use ffmpeg::util::format::pixel::Pixel;
use ffmpeg::frame;
use image::RgbaImage;

use nav::NavigationUi;

pub mod line_graph;
pub mod nav;

fn main() {
    ffmpeg::init().unwrap();
    ffmpeg::format::network::init();

    let opengl = OpenGL::_3_2;
    let window = Sdl2Window::new(
        WindowSettings::new(
            "PISCES Rover Controller".to_string(),
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
    
    let mut nav_ui = NavigationUi::new(socket);
    nav_ui.send_rpm();
    nav_ui.send_f_pan();
    nav_ui.send_f_tilt();
    
    ////////////////////////////////////////////////////////////////////////////////////////
    
    let rgba_img = RgbaImage::new(512, 512);
    let mut video_texture = Texture::from_image(&rgba_img);
    let rgba_img = Arc::new(Mutex::new(rgba_img));
    
    let thread_rgba_img = rgba_img.clone();
    thread::Builder::new()
        .name("video_packet_in".to_string())
        .spawn(move || {
            let mut format_context = format::open(&"rtsp://10.10.153.26/axis-media/media.amp".to_string()).unwrap();
            //let mut format_context = format::open(&"rtsp://192.168.1.117:554/ch0_0.h264".to_string()).unwrap();
            
            unsafe {
                println!("ANALYZE {}", (*format_context.as_mut_ptr()).max_analyze_duration);
                println!("ANALYZE2 {}", (*format_context.as_mut_ptr()).max_analyze_duration2);
                (*format_context.as_mut_ptr()).max_analyze_duration = 100000;
                (*format_context.as_mut_ptr()).max_analyze_duration2 = 100000;
            }
            
            format::dump(&format_context, 0, Some("rtsp://10.10.153.26/axis-media/media.amp"));
            //format::dump(&format_context, 0, Some("rtsp://192.168.1.117:554/ch0_0.h264"));
            
            let stream_codec =
                format_context.streams()
                              .filter(|stream| stream.codec().medium() == media::Type::Video)
                              .next().expect("No video streams in stream")
                              .codec();
            let video_codec = codec::decoder::find(stream_codec.id()).unwrap();
            
            let codec_context = stream_codec.clone().open(&video_codec).unwrap();
            
            let mut decoder = codec_context.decoder().unwrap().video().unwrap();
            let mut sws_context = scaling::Context::get(decoder.format(), decoder.width(), decoder.height(),
                                                    Pixel::RGBA, 512, 512,
                                                    scaling::flag::BILINEAR).unwrap();
            
            let mut input_frame = frame::Video::new(decoder.format(), decoder.width(), decoder.height());
            let mut output_frame = frame::Video::new(Pixel::RGBA, 512, 512);
            
            for (_, packet) in format_context.packets() {
                decoder.decode(&packet, &mut input_frame).unwrap();
                
                sws_context.run(&input_frame, &mut output_frame).unwrap();
                
                //let mut buf: Vec<u8> = Vec::with_capacity(1048576);
                for line in output_frame.data().iter() {
                    let mut rgba_img = thread_rgba_img.lock().unwrap();
                
                    //buf.reserve(line.len());
                    unsafe {
                        //let buf_len = buf.len();
                        //buf.set_len(buf_len + line.len());
                        let src: *const u8 = std::mem::transmute(line.get(0));
                        //let dst: *mut u8 = std::mem::transmute(buf.get_mut(buf_len));
                        let dst = rgba_img.as_mut_ptr();
                        std::ptr::copy(src, dst, line.len());
                    }
                }
            }
        }).unwrap();
    
    ///////////////////////////////////////////////////////////////////////////////////////

    for e in event_iter {
        ui.handle_event(&e);
        
        e.press(|button| {
            match button {
                input::Button::Keyboard(key) => nav_ui.on_key_pressed(key), 
                _ => { },
            }
        });
        
        e.release(|button| {
            match button {
                input::Button::Keyboard(key) => nav_ui.on_key_released(key), 
                _ => { },
            }
        });
        
        // Update
        e.update(|_| {
            while let Ok(packet) = packet_r.try_recv() {
                nav_ui.handle_packet(packet);
            }
            
            if let Some(ref controller) = controller {
                // Control RPM with analog sticks
                let left_y = controller.get_axis(controller::Axis::LeftY).unwrap();
                let right_y = controller.get_axis(controller::Axis::RightY).unwrap();
                
                let l_rpm = -(left_y as f32 / 32768.0) * nav_ui.max_rpm;
                let r_rpm = -(right_y as f32 / 32768.0) * nav_ui.max_rpm;
                
                nav_ui.try_update_rpm(l_rpm, r_rpm);
                
                // Control pan with left/right arrow keys
                if controller.get_button(controller::Button::DPadLeft).unwrap() {
                    nav_ui.f_pan -= f32::min(5.0, nav_ui.f_pan - 0.0);
                    nav_ui.send_f_pan();
                }
                if controller.get_button(controller::Button::DPadRight).unwrap() {
                    nav_ui.f_pan += f32::min(5.0, 180.0 - nav_ui.f_pan);
                    nav_ui.send_f_pan();
                }
                
                // Control tilt with up/down arrow keys
                if controller.get_button(controller::Button::DPadDown).unwrap() {
                    nav_ui.f_tilt -= f32::min(5.0, nav_ui.f_tilt - 90.0);
                    nav_ui.send_f_tilt();
                }
                if controller.get_button(controller::Button::DPadUp).unwrap() {
                    nav_ui.f_tilt += f32::min(5.0, 180.0 - nav_ui.f_tilt);
                    nav_ui.send_f_tilt();
                }
            }
            
            let rgba_img = rgba_img.lock().unwrap();
            video_texture.update(&*rgba_img);
        });
        
        // Render GUI
        e.render(|args| {
            gl.draw(args.viewport(), |c, gl| {
                use graphics::*;
            
                nav_ui.draw_ui(c, gl, &mut ui);
                
                Rectangle::new([0.0, 0.0, 0.4, 1.0])
                    .draw([1280.0 - 700.0 - 5.0, 5.0, 700.0, 400.0],
                          &c.draw_state, c.transform,
                          gl);
                image(&video_texture, c.scale(700.0/512.0, 400.0/512.0).trans(1280.0 - 700.0 - 5.0, 5.0).transform, gl);
                
                Rectangle::new([0.0, 0.0, 0.4, 1.0])
                    .draw([1280.0 - 700.0 - 10.0, 495.0, 350.0, 200.0],
                          &c.draw_state, c.transform,
                          gl);
                Rectangle::new([0.0, 0.0, 0.4, 1.0])
                    .draw([1280.0 - 350.0 - 5.0, 495.0, 350.0, 200.0],
                          &c.draw_state, c.transform,
                          gl);
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