use std::cell::RefCell;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::mem;
use std::net::UdpSocket;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::channel;
use std::thread;

extern crate time;
extern crate piston_window;
extern crate graphics;
extern crate image;
extern crate gfx_graphics;
extern crate gfx_device_gl;
#[macro_use] extern crate conrod;
#[macro_use] extern crate ffmpeg;

use conrod::Theme;
use piston_window::{EventLoop, Glyphs, PistonWindow, WindowSettings};

use conrod_config::Ui;
use nav_ui::NavigationUi;
use video_stream::{init_ffmpeg, start_video_stream, VideoMsg};

use image::imageops::FilterType;

mod conrod_config;
mod nav_ui;
mod video_stream;
mod imu;

fn main() {
    init_ffmpeg();

    let ref mut window: PistonWindow = WindowSettings::new("PISCES Navigation".to_string(),
                                                           [1280, 700]).exit_on_esc(true)
                                                                       .build().unwrap();

    let font_path = Path::new("./assets/fonts/NotoSans-Regular.ttf");
    let mut glyph_cache = conrod::backend::piston_window::GlyphCache::new(window, 1280, 700);
    let mut ui = {
        let theme = Theme::default();
        conrod::UiBuilder::new().theme(theme).build()
    };

    ui.fonts.insert_from_file(font_path).unwrap();
    
    // Create a UDP socket to talk to the rover
    let client = UdpSocket::bind("0.0.0.0:30002").unwrap();
    client.send_to(b"connect me plz", ("10.10.153.8", 30001));
    
    let client_in = client.try_clone().unwrap();
    let (packet_t, packet_r) = channel();

    /*let mut client = TcpStream::connect("10.10.153.8:30001").unwrap();
    client.write(b"connect me plz");
    
    let mut client_in = client.try_clone().unwrap();
    let (packet_t, packet_r) = channel();*/
    
    thread::Builder::new()
        .name("packet_in".to_string())
        .spawn(move || {
            let mut buf = [0u8; 512];
            loop {
                let (bytes_read, _) = client_in.recv_from(&mut buf).unwrap();
                //let bytes_read = client_in.read(&mut buf).unwrap();
                if let Ok(msg) = String::from_utf8(buf[0..bytes_read].iter().cloned().collect()) {
                    packet_t.send(msg).unwrap();
                }
            }
        }).unwrap();

    ////////////////////////////////////////////////////////////////////////////////////////

    let mission_folder = format!("{}", time::now().strftime("%Y%b%d_%H_%M").unwrap());
    fs::create_dir_all(format!("mission_data/{}", mission_folder.as_str()).as_str());

    let (vid0_t, vid0_r) = channel();
    let (vid1_t, vid1_r) = channel();
    let (vid2_t, vid2_r) = channel();
    
    let (video0_texture, video0_image) =
        start_video_stream(window, Some(vid0_r), "rtsp://10.10.153.9/axis-media/media.amp", 450);
    let (video1_texture, video1_image) =
        start_video_stream(window, Some(vid1_r), "rtsp://10.10.153.10/axis-media/media.amp", 450);
    let (video2_texture, video2_image) =
        start_video_stream(window, Some(vid2_r), "rtsp://root:pisces@10.10.153.11/axis-media/media.amp", 450);

    ///////////////////////////////////////////////////////////////////////////////////////
    
    let mut nav_ui = NavigationUi::new(client, vid0_t, vid1_t, vid2_t, mission_folder.clone());
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

    window.set_ups(10);
    window.set_max_fps(60);

    let mut snapshot_num = 0;

    while let Some(e) = window.next() {
        use piston_window::{Button, PressEvent, ReleaseEvent, UpdateEvent, MouseCursorEvent};

        // Convert the piston event to a conrod event.
        if let Some(e) = conrod::backend::piston_window::convert_event(e.clone(), window) {
            ui.handle_event(e);
        }

        e.mouse_cursor(|x, y| {
            mouse_x = x;
            mouse_y = y;
        });
        
        e.press(|button| {
            match button {
                Button::Keyboard(key) => nav_ui.on_key_pressed(key), 
                Button::Mouse(b) => {
                    use piston_window::mouse::MouseButton;
                    if b == MouseButton::Left {
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
                _ => { },
            }
        });
        
        e.release(|button| {
            match button {
                Button::Keyboard(key) => nav_ui.on_key_released(key), 
                _ => { },
            }
        });
        
        // Update
        e.update(|u_args| {
            nav_ui.update(u_args.dt);

            while let Ok(packet) = packet_r.try_recv() {
                nav_ui.handle_packet(packet);
            }
            
            let video0_image = video0_image.lock().unwrap();
            vid_textures[0].update(&mut window.encoder, &video0_image.as_rgba8().unwrap());
            
            let video1_image = video1_image.lock().unwrap();
            vid_textures[1].update(&mut window.encoder, &video1_image.as_rgba8().unwrap());
            
            let video2_image = video2_image.lock().unwrap();
            vid_textures[2].update(&mut window.encoder, &video2_image.as_rgba8().unwrap());

            if nav_ui.want_snapshot {
                nav_ui.want_snapshot = false;
                let snapshot_file_name = format!("mission_data/{}/snapshot_{}.jpg", mission_folder.as_str(), snapshot_num);
                let ref mut fout = File::create(&Path::new(&snapshot_file_name)).unwrap();
                snapshot_num += 1;
                let img =
                    match vid_displays[0] {
                        0 => { video0_image.resize_exact(700, 400, FilterType::Nearest) },
                        1 => { video1_image.resize_exact(700, 400, FilterType::Nearest) },
                        2 => { video2_image.resize_exact(700, 400, FilterType::Nearest) },
                        _ => { unreachable!(); },
                    };
                img.save(fout, image::JPEG).unwrap();
            }
        });

        // Render GUI
        window.draw_2d(&e, |c, g| {
            use graphics::*;

            nav_ui.draw_ui(c, g, &mut glyph_cache, &mut ui);

            Rectangle::new([0.0, 0.0, 0.4, 1.0])
                .draw([1280.0 - 700.0 - 5.0, 5.0, 700.0, 400.0],
                      &c.draw_state, c.transform,
                      g);
            image(&vid_textures[vid_displays[0]],
                  c.trans(1280.0 - 700.0 - 5.0, 5.0).scale(700.0/450.0, 400.0/450.0).transform, g);
            
            Rectangle::new([0.0, 0.0, 0.4, 1.0])
                .draw([1280.0 - 700.0 - 10.0, 495.0, 350.0, 200.0],
                      &c.draw_state, c.transform,
                      g);
            image(&vid_textures[vid_displays[1]],
                  c.trans(1280.0 - 700.0 - 10.0, 495.0).scale(350.0/450.0, 200.0/450.0).transform, g);
            
            Rectangle::new([0.0, 0.0, 0.4, 1.0])
                .draw([1280.0 - 350.0 - 5.0, 495.0, 350.0, 200.0],
                      &c.draw_state, c.transform,
                      g);
            image(&vid_textures[vid_displays[2]],
                  c.trans(1280.0 - 350.0 - 5.0, 495.0).scale(350.0/450.0, 200.0/450.0).transform, g);
        });
    }
}
