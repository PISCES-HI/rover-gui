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

pub fn start_video_stream(path: &str, out_path: Option<String>) -> (Texture, Arc<Mutex<RgbaImage>>) {
    let rgba_img = RgbaImage::new(512, 512);
    let video_texture = Texture::from_image(&rgba_img);
    let rgba_img = Arc::new(Mutex::new(rgba_img));

    let path = path.to_string();
    
    let thread_rgba_img = rgba_img.clone();
    thread::Builder::new()
        .name("video_packet_in".to_string())
        .spawn(move || {
            let mut format_context = format::input(&path).unwrap();
            //format::dump(&format_context, 0, Some(path.as_str()));

            let stream_codec =
                format_context.streams()
                              .filter(|stream| stream.codec().medium() == media::Type::Video)
                              .map(|stream| stream.codec())
                              .next().expect("No video streams in stream");
            let video_codec = codec::decoder::find(stream_codec.id()).unwrap();
            
            let codec_context = stream_codec.clone();

            let mut decoder = codec_context.decoder().video().unwrap();
            let mut sws_context = scaling::Context::get(decoder.format(), decoder.width(), decoder.height(),
                                                    Pixel::RGBA, 512, 512,
                                                    scaling::flag::BILINEAR).unwrap();

            // Open recording stream
            let (video_t, video_r) = channel();
            if let Some(ref out_path) = out_path {
                start_video_recording(&decoder, video_r, out_path.clone());
            }

            /////////////////////////////////////////////////////
            // Process stream
            
            for (stream, packet) in format_context.packets() {
                let now = ffmpeg::time::relative() as i64;

                let mut input_frame = frame::Video::new(decoder.format(), decoder.width(), decoder.height());
                let mut output_frame = frame::Video::new(Pixel::RGBA, 512, 512);

                decoder.decode(&packet, &mut input_frame).unwrap();
                
                if let Err(e) = sws_context.run(&input_frame, &mut output_frame) {
                    println!("WARNING: video software scaling error: {}", e);
                }
                
                //let mut buf: Vec<u8> = Vec::with_capacity(1048576);
                for line in output_frame.data().iter() {
                    let mut rgba_img = thread_rgba_img.lock().unwrap();
                
                    //buf.reserve(line.len());
                    unsafe {
                        //let buf_len = buf.len();
                        //buf.set_len(buf_len + line.len());
                        let src: *const u8 = mem::transmute(line.get(0));
                        //let dst: *mut u8 = std::mem::transmute(buf.get_mut(buf_len));
                        let dst = rgba_img.as_mut_ptr();
                        ptr::copy(src, dst, line.len());
                    }
                }

                if out_path.is_some() {
                    video_t.send((now, input_frame));
                }
            }
        }).unwrap();
    
    (video_texture, rgba_img)
}

pub fn start_video_recording(decoder: &ffmpeg::codec::decoder::Video,
                             packets: Receiver<(i64, ffmpeg::frame::Video)>,
                             out_path: String) {
    let decoder_width = decoder.width();
    let decoder_height = decoder.height();
    let decoder_format = decoder.format();
    
    thread::Builder::new()
        .name("video_packet_in".to_string())
        .spawn(move || {
            let fps: i64 = 10;

            /////////////////////////////////////////////////////
            // Open recording stream

            let mut rec_format = ffmpeg::format::output(&format!("{}", out_path)).unwrap();

            let mut rec_video = {
                    let mut stream = rec_format.add_stream(ffmpeg::codec::Id::VP9).unwrap();
                    let mut codec  = stream.codec().encoder().video().unwrap();

                    codec.set_width(decoder_width);
                    codec.set_height(decoder_height);
                    codec.set_format(ffmpeg::format::Pixel::YUV420P);
                    //codec.set_format(decoder.format());
                    codec.set_time_base((1, fps as i32));
                    codec.set_flags(ffmpeg::codec::flag::GLOBAL_HEADER);

                    stream.set_time_base((1, 1_000));
                    stream.set_rate((fps as i32, 1));

                    codec.open_as(ffmpeg::codec::Id::VP9).unwrap()
                    //codec.open().unwrap()
                    //codec.encoder()
            };

            let mut rec_converter =
                ffmpeg::software::converter((decoder_width, decoder_height),
                                            decoder_format,
                                            ffmpeg::format::Pixel::YUV420P).unwrap();

            rec_format.write_header().unwrap();

            let mut rec_packet = ffmpeg::Packet::empty();
            let mut rec_frame  = ffmpeg::frame::Video::empty();

            let start = ffmpeg::time::relative() as i64;
            let sleep = 1_000_000/fps;

            /////////////////////////////////////////////////////
            // Process streams
            
            while let Ok((now, input_frame)) = packets.recv() {
                // Now encode the recording packets
                if let Err(e) = rec_converter.run(&input_frame, &mut rec_frame) {
                    println!("WARNING: video software converter error: {}", e);
                }
                rec_frame.set_pts(Some((now - start) / sleep));

                //println!("encoding...");
                if let Ok(_) = rec_video.encode(&rec_frame, &mut rec_packet) {
                    rec_packet.set_stream(0);
                    rec_packet.rescale_ts((1, fps as i32), (1, 1_000));
                    rec_packet.write_interleaved(&mut rec_format);
                } else {
                    println!("WARNING: Failed to write video frame");
                }
            }

            rec_format.write_trailer().unwrap();
        }).unwrap();
}

pub fn init_ffmpeg() {
    ffmpeg::init().unwrap();
    ffmpeg::format::network::init();
}
