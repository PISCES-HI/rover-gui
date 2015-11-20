use std::fs::File;
use std::io::{
    BufWriter,
    Write,
};
use std::mem;
use std::path::Path;
use std::ptr;
use std::slice;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;

use ffmpeg;
use ffmpeg::codec;
use ffmpeg::format;
use ffmpeg::media;
use ffmpeg::frame;
use ffmpeg::software::scaling;
use ffmpeg::util::format::pixel::Pixel;
use image::RgbaImage;

use opengl_graphics::Texture;

pub enum VideoMsg {
    Start(String),
    Stop,
}

pub fn start_video_stream(record_r: Receiver<VideoMsg>,
                          path: &str) -> (Texture, Arc<Mutex<RgbaImage>>) {
    let rgba_img = RgbaImage::new(512, 512);
    let video_texture = Texture::from_image(&rgba_img);
    let rgba_img = Arc::new(Mutex::new(rgba_img));

    let path = path.to_string();
    
    let thread_rgba_img = rgba_img.clone();
    thread::Builder::new()
        .name("video_packet_in".to_string())
        .spawn(move || {
            let fps: i64 = 10;

            let mut format_context = format::input(&path).unwrap();
            //format::dump(&format_context, 0, Some(path.as_str()));

            let (start_time, stream_codec) =
                format_context.streams()
                              .filter(|stream| stream.codec().medium() == media::Type::Video)
                              .map(|stream| (stream.start_time(), stream.codec()))
                              .next().expect("No video streams in stream");
            let video_codec = codec::decoder::find(stream_codec.id()).unwrap();
            
            let codec_context = stream_codec.clone();

            let mut decoder = codec_context.decoder().video().unwrap();
            let mut sws_context = scaling::Context::get(decoder.format(), decoder.width(), decoder.height(),
                                                    Pixel::RGBA, 512, 512,
                                                    scaling::flag::BILINEAR).unwrap();

            // Open recording stream
            let mut video_t: Option<Sender<RecordPacket>> = None;

            /////////////////////////////////////////////////////
            // Process stream

            let mut start = ffmpeg::time::relative() as i64;
            let sleep = 1_000_000/fps;
            
            let mut rec_start_pts = 0;
            
            for (stream, packet) in format_context.packets() {
                let mut input_frame = frame::Video::new(decoder.format(), decoder.width(), decoder.height());
                let mut output_frame = frame::Video::new(Pixel::RGBA, 512, 512);

                decoder.decode(&packet, &mut input_frame).unwrap();
                
                if let Err(e) = sws_context.run(&input_frame, &mut output_frame) {
                    println!("WARNING: video software scaling error: {}", e);
                }
                
                // Copy frame data to the rgba_img
                {
                    let frame_data = output_frame.data(0);
                    let mut rgba_img = thread_rgba_img.lock().unwrap();
                    unsafe {
                        let src: *const u8 = mem::transmute(frame_data.get(0));
                        let dst = rgba_img.as_mut_ptr();
                        ptr::copy(src, dst, frame_data.len());
                    }
                }

                // Check for messages
                if let Ok(msg) = record_r.try_recv() {
                    match msg {
                        VideoMsg::Start(out_path) => {
                            // Open recording stream
                            if video_t.is_none() {
                                rec_start_pts = packet.pts().unwrap();
                                start = ffmpeg::time::relative() as i64;
                                let (t, r) = channel();
                                start_video_recording(&decoder, r, out_path);
                                video_t = Some(t);
                            }
                        },
                        VideoMsg::Stop => {
                            if let Some(ref video_t) = video_t {
                                video_t.send(RecordPacket::Close);
                            }
                            video_t = None;
                        },
                    }
                }

                if let Some(ref video_t) = video_t {
                    /*let pts = packet.pts()
                                    .unwrap_or(((ffmpeg::time::relative() as i64) - start)/sleep);*/
                    let pts = ((ffmpeg::time::relative() as i64) - start)/sleep;
                    //let pts = (input_frame.timestamp().unwrap()-start_time)/sleep;
                    /*let pts = packet.pts().unwrap();
                    let pts =
                        if pts > rec_start_pts {
                            pts - rec_start_pts
                        } else {
                            0
                        };*/
                    println!("PTS {}, {:?}, {}", pts, packet.pts().unwrap()/10_000, packet.position());
                    video_t.send(RecordPacket::Packet(pts, input_frame));
                }
            }
        }).unwrap();
    
    (video_texture, rgba_img)
}

enum RecordPacket {
    Packet(i64, ffmpeg::frame::Video),
    Close,
}

fn start_video_recording(decoder: &ffmpeg::codec::decoder::Video,
                         msgs: Receiver<RecordPacket>,
                         out_path: String) {
    let decoder_width = decoder.width();
    let decoder_height = decoder.height();
    let decoder_format = decoder.format();

    println!("time_base={}", decoder.time_base());
    
    thread::Builder::new()
        .name("video_packet_in".to_string())
        .spawn(move || {
            let fps: i64 = 10;

            /////////////////////////////////////////////////////
            // Open recording stream

            let mut rec_format = ffmpeg::format::output(&format!("{}", out_path)).unwrap();

            let mut rec_video = {
                    let mut stream = rec_format.add_stream(ffmpeg::codec::Id::MPEG4).unwrap();
                    let mut codec  = stream.codec().encoder().video().unwrap();

                    codec.set_width(decoder_width);
                    codec.set_height(decoder_height);
                    codec.set_format(ffmpeg::format::Pixel::YUV420P);
                    //codec.set_time_base((1, fps as i32));
                    codec.set_time_base((1, 1000));
                    codec.set_flags(ffmpeg::codec::flag::GLOBAL_HEADER);

                    stream.set_time_base((1, 1000));
                    //stream.set_time_base((1, fps as i32));
                    //stream.set_rate((fps as i32, 1));

                    codec.open_as(ffmpeg::codec::Id::MPEG4).unwrap()
            };

            let mut rec_converter =
                ffmpeg::software::converter((decoder_width, decoder_height),
                                            decoder_format,
                                            ffmpeg::format::Pixel::YUV420P).unwrap();

            rec_format.write_header().unwrap();

            let mut rec_packet = ffmpeg::Packet::empty();
            let mut rec_frame  = ffmpeg::frame::Video::empty();

            /////////////////////////////////////////////////////
            // Process streams
            
            while let Ok(msg) = msgs.recv() {
                match msg {
                    RecordPacket::Packet(pts, input_frame) => {
                        // Now encode the recording packets
                        if let Err(e) = rec_converter.run(&input_frame, &mut rec_frame) {
                            println!("WARNING: video software converter error: {}", e);
                        }
                        rec_frame.set_pts(Some(pts));

                        match rec_video.encode(&rec_frame, &mut rec_packet) {
                            Ok(_) => {
                                rec_packet.set_stream(0);
                                rec_packet.rescale_ts((1, 10), (1, 17500));
                                rec_packet.write_interleaved(&mut rec_format);
                            },
                            Err(e) => {
                                println!("WARNING: Failed to write video frame: {}", e);
                            },
                        }
                    },
                    RecordPacket::Close => {
                        break;
                    },
                }
            }

            while let Ok(true) = rec_video.flush(&mut rec_packet) {
                rec_packet.set_stream(0);
                rec_packet.rescale_ts((1, 10), (1, 17500));
                rec_packet.write_interleaved(&mut rec_format);
            }

            rec_format.write_trailer().unwrap();
            println!("Finished writing trailer");
        }).unwrap();
}

pub fn init_ffmpeg() {
    ffmpeg::init().unwrap();
    ffmpeg::format::network::init();
}
