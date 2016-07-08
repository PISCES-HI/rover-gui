use std::collections::{HashMap, VecDeque};
use std::io;
use std::io::Write;
use std::net::UdpSocket;
use std::ops::DerefMut;
use std::sync::mpsc::Sender;

use conrod::{
    self,
    Background,
    Button,
    Color,
    Colorable,
    Frameable,
    Text,
    Labelable,
    Positionable,
    Slider,
    Sizeable,
    TextBox,
    Ui,
    Widget,
};
use conrod::color::{rgb, WHITE};
use graphics::{Context, Graphics};
use piston_window::{self, Key};
use time;

use conrod_config;
use imu;
use video_stream::VideoMsg;

enum MissionTime {
    Paused(time::Duration),
    Running(time::Tm, time::Duration),
}

pub struct StereoUi {
    bg_color: Color,

    mission_time: MissionTime,

    // Forward camera controls
    pub pan: f32,
    pub panning: f32,
    pub last_pan_time: time::Tm,
    pub tilt: f32,
    pub tilting: f32,
    pub last_tilt_time: time::Tm,

    client: UdpSocket,
    
    out_queue: VecDeque<(time::Tm, time::Duration, Vec<u8>, (String, u16))>, // Outbound packet queue
    delay: time::Duration,
    delay_str: String,
}

impl StereoUi {
    pub fn new(client: UdpSocket) -> StereoUi {
        StereoUi {
            bg_color: rgb(0.2, 0.35, 0.45),

            mission_time: MissionTime::Paused(time::Duration::zero()),

            pan: 90.0,
            panning: 0.0,
            last_pan_time: time::now(),
            tilt: 90.0,
            tilting: 0.0,
            last_tilt_time: time::now(),

            client: client,

            out_queue: VecDeque::new(),
            delay: time::Duration::seconds(0),
            delay_str: "".to_string(),
        }
    }

    pub fn update(&mut self, dt: f64) {
        let dt = dt as f32;

        self.pan += self.panning*180.0*dt; // 180 degrees per second
        self.tilt += self.tilting*90.0*dt; // 90 degrees per second

        self.flush_out_queue();
    }

    pub fn draw_ui<'a, G>(&mut self, c: Context, g: &mut G, ui: &mut conrod_config::Ui)
                          where G: Graphics<Texture=<piston_window::G2d<'static> as conrod::Graphics>::Texture> {
        use graphics::Transformed;

        // Draw the background.
        Background::new().color(self.bg_color).set(ui);

        // Draw our UI!
        ui.draw(c, g);
    }

    pub fn set_widgets(&mut self, ui: &mut conrod_config::UiCell) {
        let time_now = time::now();

        // Local time
        Text::new(format!("{}", time_now.strftime("Local  %x  %X").unwrap()).as_str())
            .x_y((-ui.win_w / 2.0) + 100.0, (ui.win_h / 2.0) - 10.0)
            .font_size(16)
            .color(self.bg_color.plain_contrast())
            .set(LOCAL_TIME, ui);

        // UTC time
        Text::new(format!("{}", time_now.to_utc().strftime("%Z  %x  %X").unwrap()).as_str())
            .x_y((-ui.win_w / 2.0) + 104.0, (ui.win_h / 2.0) - 30.0)
            .font_size(16)
            .color(self.bg_color.plain_contrast())
            .set(UTC_TIME, ui);

        ////////////////////////////////////////////////////////////////////////////////////////////
        
        // Camera pan slider
        Slider::new(self.pan, 0.0, 180.0)
            .w_h(150.0, 30.0)
            .x_y(-80.0, (ui.win_h / 2.0) - 600.0)
            .rgb(0.5, 0.3, 0.6)
            .frame(1.0)
            .label("Pan")
            .label_color(WHITE)
            .react(|new_pan| {
                self.try_update_pan(new_pan);
            })
            .set(F_PAN_SLIDER, ui);

        // Camera tilt slider
        Slider::new(self.tilt, 0.0, 180.0)
            .w_h(150.0, 30.0)
            .x_y(80.0, (ui.win_h / 2.0) - 600.0)
            .rgb(0.5, 0.3, 0.6)
            .frame(1.0)
            .label("Tilt")
            .label_color(WHITE)
            .react(|new_tilt| {
                self.try_update_tilt(new_tilt);
            })
            .set(F_TILT_SLIDER, ui);

        ////////////////////////////////////////////////////////////////////////////////////////////
       
        Button::new()
            .w_h(120.0, 30.0)
            .x_y(- 80.0, (ui.win_h / 2.0) - 645.0)
            .rgb(0.3, 0.8, 0.3)
            .frame(1.0)
            .label("Snapshot")
            .react(|| { self.send_snapshot(); })
            .set(SNAPSHOT_BUTTON, ui);
    }

    pub fn handle_packet(&mut self, packet: String) {
        //println!("{}", packet);

        let packets = packet.split("|");

        for packet in packets {
            let packet_parts: Vec<String> = packet.split(":").map(|s| s.to_string()).collect();

            match packet_parts[0].as_str() {
                _ => { /*println!("WARNING: Unknown packet ID: {}", packet_parts[0])*/ },
            }
        }
    }

    pub fn on_key_pressed<'a>(&mut self, key: Key) {
        use piston_window::Key::*;

        match key {
            W => {
                // Camera up
                self.tilting = 1.0;
            },
            S => {
                // Camera down
                self.tilting = -1.0;
            },
            A => {
                // Camera left
                self.panning = -1.0;
            },
            D => {
                // Camera right
                self.panning = 1.0;
            },
            _ => { },
        }
    }

    pub fn on_key_released<'a>(&mut self, key: Key) {
        use piston_window::Key::*;

        match key {
            W | S => {
                self.tilting = 0.0;
                self.send_tilt();
            },
            A | D => {
                self.panning = 0.0;
                self.send_pan();
            },
            _ => { },
        }
    }

    pub fn try_update_pan(&mut self, pan: f32) {
        if (pan - self.pan).abs() > 5.0 || pan == 0.0 || pan == 180.0 {
            self.pan = pan;
            self.send_pan();
        }
    }

    pub fn try_update_tilt(&mut self, tilt: f32) {
        if (tilt - self.tilt).abs() > 5.0 || tilt == 90.0 || tilt == 180.0 {
            self.tilt = tilt;
            self.send_tilt();
        }
    }
    
    pub fn send_snapshot(&mut self) {
        let time_since = (time::now() - self.last_pan_time).num_milliseconds();
        if time_since >= 500 {
            self.last_pan_time = time::now();
            let packet = format!("K|");
            let delay = self.delay;
            self.queue_packet(delay, packet.into_bytes(), ("10.10.155.165".to_string(), 30001));
        }
    }

    pub fn send_pan(&mut self) {
        let time_since = (time::now() - self.last_pan_time).num_milliseconds();
        if time_since >= 500 {
            self.last_pan_time = time::now();
            let packet = format!("I{}|", self.pan as i32);
            let delay = self.delay;
            self.queue_packet(delay, packet.into_bytes(), ("10.10.155.165".to_string(), 30001));
        }
    }

    pub fn send_tilt(&mut self) {
        let time_since = (time::now() - self.last_tilt_time).num_milliseconds();
        if time_since >= 500 {
            self.last_tilt_time = time::now();
            let packet = format!("J{}|", self.tilt as i32);
            let delay = self.delay;
            self.queue_packet(delay, packet.into_bytes(), ("10.10.155.165".to_string(), 30001));
        }
    }

    pub fn queue_packet(&mut self, delay: time::Duration, mut data: Vec<u8>, addr: (String, u16)) {
        data.push(0); // Null terminate all of our packets
        self.out_queue.push_back((time::now(), delay, data, addr));
    }

    fn flush_out_queue(&mut self) -> io::Result<usize> {
        use std::iter;

        let mut bytes_written = 0;
        while !self.out_queue.is_empty() {
            if time::now()-self.out_queue[0].0 >= self.out_queue[0].1 {
                let (_, _, mut data, addr) = self.out_queue.pop_front().unwrap();
                let data_len = data.len();
                bytes_written += try!(self.client.send_to(data.as_slice(), (addr.0.as_str(), addr.1)));
                //data.extend(iter::repeat(b' ').take(64 - data_len)); // Pad the message to always be 64 bytes
                //bytes_written += try!(self.client.write(data.as_slice()));
            } else {
                break;
            }
        }
        Ok(bytes_written)
    }
}

widget_ids! {
    LOCAL_TIME,
    UTC_TIME,
    MISSION_TIME_LABEL,
    TIME_DELAY,
    TIME_DELAY_VALUE,

    F_PAN_SLIDER,
    F_TILT_SLIDER,
    SNAPSHOT_BUTTON,
    MODE_LABEL,
    MODE_TOGGLE_BUTTON,
}
