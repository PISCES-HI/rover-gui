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
use std::thread;

use ffmpeg;
use ffmpeg::codec;
use ffmpeg::format;
use ffmpeg::media;
use ffmpeg::software::scaling;
use ffmpeg::util::format::pixel::Pixel;
use ffmpeg::frame;
use image::RgbaImage;

use opengl_graphics::Texture;

pub fn start_video_stream(path: &str, out_path: Option<&str>) -> (Texture, Arc<Mutex<RgbaImage>>) {
    let rgba_img = RgbaImage::new(512, 512);
    let video_texture = Texture::from_image(&rgba_img);
    let rgba_img = Arc::new(Mutex::new(rgba_img));

    let mut out_file = out_path.map(|p| BufWriter::new(File::create(p).unwrap()));
    if let Some(ref mut out_file) = out_file {
        out_file.write_all(&[0, 0, 0, 1]);
    }

    let path = path.to_string();
    
    let thread_rgba_img = rgba_img.clone();
    thread::Builder::new()
        .name("video_packet_in".to_string())
        .spawn(move || {
            let mut format_context = format::open(&path).unwrap();
            format::dump(&format_context, 0, Some(path.as_str()));

            let stream_codec =
                format_context.streams()
                              .filter(|stream| stream.codec().medium() == media::Type::Video)
                              .map(|stream| stream.codec())
                              .next().expect("No video streams in stream");
            let video_codec = codec::decoder::find(stream_codec.id()).unwrap();
            
            let codec_context = stream_codec.clone().open(&video_codec).unwrap();
            
            let mut decoder = codec_context.decoder().unwrap().video().unwrap();
            let mut sws_context = scaling::Context::get(decoder.format(), decoder.width(), decoder.height(),
                                                    Pixel::RGBA, 512, 512,
                                                    scaling::flag::BILINEAR).unwrap();
            
            let mut input_frame = frame::Video::new(decoder.format(), decoder.width(), decoder.height());
            let mut output_frame = frame::Video::new(Pixel::RGBA, 512, 512);
            
            for (stream, packet) in format_context.packets() {
                // If out_file exists, record video to it
                if let Some(ref mut out_file) = out_file {
                    unsafe {
                        let data: &[u8] = slice::from_raw_parts((*packet.as_ptr()).data, packet.size());
                        out_file.write_all(data);
                    }
                }

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
            }
        }).unwrap();
    
    (video_texture, rgba_img)
}

pub fn init_ffmpeg() {
    ffmpeg::init().unwrap();
    ffmpeg::format::network::init();
}
