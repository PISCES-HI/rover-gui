#![feature(convert)]

use std::fs;
use std::mem;
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

use nav_ui::NavigationUi;
use video_stream::{init_ffmpeg, start_video_stream, RecordMsg};

mod line_graph;
mod nav_ui;
mod video_stream;
mod imu;

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
    let socket = UdpSocket::bind("0.0.0.0:30002").unwrap();
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

    ////////////////////////////////////////////////////////////////////////////////////////

    let mission_folder = format!("{}", time::now().strftime("%Y%b%d_%H_%M").unwrap());
    fs::create_dir_all(format!("mission_data/{}", mission_folder).as_str());

    let (vid0_t, vid0_r) = channel();
    let (vid1_t, vid1_r) = channel();
    let (vid2_t, vid2_r) = channel();
    
    let (video0_texture, video0_image) =
        start_video_stream(vid0_r,
                           "rtsp://10.10.155.166/axis-media/media.amp",
                           Some(format!("mission_data/{}/forward.mkv", mission_folder)));
    let (video1_texture, video1_image) =
        start_video_stream(vid1_r,
                           "rtsp://10.10.155.167/axis-media/media.amp",
                           Some(format!("mission_data/{}/reverse.mkv", mission_folder)));
    let (video2_texture, video2_image) =
        start_video_stream(vid2_r,
                           "rtsp://root:pisces@10.10.155.168/axis-media/media.amp",
                           Some(format!("mission_data/{}/hazard.mkv", mission_folder)));

    ///////////////////////////////////////////////////////////////////////////////////////
    
    let mut nav_ui = NavigationUi::new(socket, vid0_t, vid1_t, vid2_t);
    nav_ui.send_l_rpm();
    nav_ui.send_r_rpm();
    nav_ui.send_f_pan();
    nav_ui.send_f_tilt();

    ////////////////////////////////////////////////////////////////////////////////////////

    let mut vid_textures = [video0_texture, video1_texture, video2_texture];
    let mut vid_displays = [0, 1, 2];

    let mut mouse_x = 0.0;
    let mut mouse_y = 0.0;
    
    ///////////////////////////////////////////////////////////////////////////////////////

    for e in event_iter {
        ui.handle_event(&e);

        e.mouse_cursor(|x, y| {
            mouse_x = x;
            mouse_y = y;
        });
        
        e.press(|button| {
            match button {
                input::Button::Keyboard(key) => nav_ui.on_key_pressed(key), 
                input::Button::Mouse(b) => {
                    if b == input::mouse::MouseButton::Left {
                        if mouse_x >= 1280.0- 700.0-10.0 && mouse_x <= 1280.0-350.0-10.0 && mouse_y >= 495.0 && mouse_y <= 695.0 {
                            let tmp = vid_displays[0];
                            vid_displays[0] = vid_displays[1];
                            vid_displays[1] = tmp;
                        } else if mouse_x >= 1280.0-350.0-5.0 && mouse_x <= 1280.0-5.0 && mouse_y >= 495.0 && mouse_y <= 695.0 {
                            let tmp = vid_displays[0];
                            vid_displays[0] = vid_displays[2];
                            vid_displays[2] = tmp;
                        }
                    }
                },
            }
        });
        
        e.release(|button| {
            match button {
                input::Button::Keyboard(key) => nav_ui.on_key_released(key), 
                _ => { },
            }
        });
        
        // Update
        e.update(|u_args| {
            nav_ui.update(u_args.dt);

            while let Ok(packet) = packet_r.try_recv() {
                nav_ui.handle_packet(packet);
            }

            if let Some(ref controller) = controller {
                // Control RPM with analog sticks
                let left_y = controller.get_axis(controller::Axis::LeftY);
                let right_y = controller.get_axis(controller::Axis::RightY);
                
                let l_rpm = -(left_y as f32 / 32768.0) * nav_ui.max_rpm;
                let r_rpm = -(right_y as f32 / 32768.0) * nav_ui.max_rpm;
                
                nav_ui.try_update_l_rpm(l_rpm);
                nav_ui.try_update_r_rpm(r_rpm);

                // Control SADL with A/Y buttons
                if controller.get_button(controller::Button::A) {
                    nav_ui.try_update_sadl(100.0);
                } else if controller.get_button(controller::Button::Y) {
                    nav_ui.try_update_sadl(-100.0);
                } else {
                    nav_ui.try_update_sadl(0.0);
                }
                
                // Control pan with left/right arrow keys
                if controller.get_button(controller::Button::DPadLeft) {
                    nav_ui.f_pan -= f32::min(5.0, nav_ui.f_pan - 0.0);
                    nav_ui.send_f_pan();
                }
                if controller.get_button(controller::Button::DPadRight) {
                    nav_ui.f_pan += f32::min(5.0, 180.0 - nav_ui.f_pan);
                    nav_ui.send_f_pan();
                }
                
                // Control tilt with up/down arrow keys
                if controller.get_button(controller::Button::DPadDown) {
                    nav_ui.f_tilt -= f32::min(5.0, nav_ui.f_tilt - 90.0);
                    nav_ui.send_f_tilt();
                }
                if controller.get_button(controller::Button::DPadUp) {
                    nav_ui.f_tilt += f32::min(5.0, 180.0 - nav_ui.f_tilt);
                    nav_ui.send_f_tilt();
                }
            }
            
            let video0_image = video0_image.lock().unwrap();
            vid_textures[0].update(&*video0_image);
            
            let video1_image = video1_image.lock().unwrap();
            vid_textures[1].update(&*video1_image);
            
            let video2_image = video2_image.lock().unwrap();
            vid_textures[2].update(&*video2_image);
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
                image(&vid_textures[vid_displays[0]], c.trans(1280.0 - 700.0 - 5.0, 5.0).scale(700.0/512.0, 400.0/512.0).transform, gl);
                
                Rectangle::new([0.0, 0.0, 0.4, 1.0])
                    .draw([1280.0 - 700.0 - 10.0, 495.0, 350.0, 200.0],
                          &c.draw_state, c.transform,
                          gl);
                image(&vid_textures[vid_displays[1]], c.trans(1280.0 - 700.0 - 10.0, 495.0).scale(350.0/512.0, 200.0/512.0).transform, gl);
                
                Rectangle::new([0.0, 0.0, 0.4, 1.0])
                    .draw([1280.0 - 350.0 - 5.0, 495.0, 350.0, 200.0],
                          &c.draw_state, c.transform,
                          gl);
                image(&vid_textures[vid_displays[2]], c.trans(1280.0 - 350.0 - 5.0, 495.0).scale(350.0/512.0, 200.0/512.0).transform, gl);
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
