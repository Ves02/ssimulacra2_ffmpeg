use bytemuck;
use clap::{arg, command, value_parser};
use ffmpeg_sidecar::command::*;
use ssimulacra2::{compute_frame_ssimulacra2, LinearRgb};
use std::io::{self, Write};
// use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if !ffmpeg_handler() {
        println!("ffmpeg is required. Exiting...");
        return Ok(());
    }

    let matches = command!()
        .arg(
            arg!(<source> "Source video or image(s)")
                .value_parser(value_parser!(String)),
        )
        .arg(
            arg!(<distorted> "Distorted video or image(s)")
                .value_parser(value_parser!(String)),
        )
        .get_matches();

    let source = matches.get_one::<String>("source").unwrap();
    // ffmpeg libswscale/output.c does not support rgbf32le, which is more ideal output format. We will have to convert to rgbf32le.
    let source_iter = FfmpegCommand::new()
        .input(source)
        .pix_fmt("gbrpf32le")
        .fps_mode("passthrough")
        .format("rawvideo")
        .pipe_stdout()
        // .print_command()
        .spawn()?
        .iter()?;

    for frame in source_iter.filter_frames() { 
        println!("{:?}", frame.data);
    }

    let distorted = matches.get_one::<String>("distorted").unwrap();
    let distorted_iter = FfmpegCommand::new()
        .input(source)
        .pix_fmt("gbrpf32le")
        .codec_video("rawvideo")
        .fps_mode("passthrough")
        .pipe_stdout()
        .print_command()
        .spawn()?
        .iter()?;

    // let mut s_frames = source_iter.filter_frames();
    let mut d_frames = distorted_iter.filter_frames();

    // while let (Some(s_frame), Some(d_frame)) = (s_frames.next(), d_frames.next()) {
    //     let s_data: &[f32] = bytemuck::cast_slice(&s_frame.data);
    //     println!("{:?}", s_data);
    //     let d_data: &[f32] = bytemuck::cast_slice(&d_frame.data);


        // let s_frame_ready = LinearRgb::new(s_data, s_frame.width, s_frame.height);
        // let d_frame_ready = LinearRgb::new(d_data, d_frame.width, d_frame.height);
    // }

    // let s_frame = source_iter.filter_frames().next().unwrap().data;
    // let s_frame: &[f32] = bytemuck::cast_slice(&s_frame);
    // let total = s_frame.len() / 3;
    // let (b_plane, rest) = s_frame.split_at(total);
    // let (g_plane, r_plane) = rest.split_at(total);

    // let rgb_pixels: Vec<[f32; 3]> = (0..total)
    //     .map(|i| [ r_plane[i], g_plane[i], b_plane[i] ])
    //     .collect();
    // let d_frame = distorted_iter.next().unwrap();

    return Ok(());
}

// old code.
// let mut bgr: [Vec<f32>; 3] = Default::default();
// for(i, chunk) in s_data.chunks_exact(3).enumerate(){
//     bgr[i] = chunk.to_vec();
// }
// let rgb = [bgr[2].clone(), bgr[1].clone(), bgr[0].clone()];
// let s_data = rgb;

// let mut bgr: [Vec<f32>; 3] = Default::default();
// for(i, chunk) in d_data.chunks_exact(3).enumerate(){
//     bgr[i] = chunk.to_vec();
// }
// let rgb = [bgr[2].clone(), bgr[1].clone(), bgr[0].clone()];
// let d_data = rgb;



fn convert_bgrpf32le_to_rgbf32le(input: Vec<u8>) -> Vec<[f32; 3]>{
    let input: &[f32] = bytemuck::cast_slice(&input);

    assert!(
        input.len() % 3 == 0,
        "Expected 3 planes of equal length, got {} floats",
        input.len()
    );

    let plane_size = input.len() / 3;

    let mut chunks = input.chunks_exact(plane_size);
    let b_plane  = chunks.next().unwrap(); 
    let g_plane = chunks.next().unwrap();  
    let r_plane   = chunks.next().unwrap(); 

    (0..plane_size)
        .map(|i| [ r_plane[i], g_plane[i], b_plane[i] ])
        .collect()
}

fn ffmpeg_handler() -> bool {
    if !ffmpeg_is_installed() {
        println!("FFmpeg is not detected on PATH.");
        print!("Install FFmpeg? (y\\n): ");
        io::stdout().flush().expect("Failed to flush stdout.");
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line.");

        if input.trim().to_lowercase() != "y" {
            return false;
        }

        ffmpeg_sidecar::download::auto_download().expect("Failed to install FFmpeg.");
        println!("FFmpeg installed! 🎉")
    } else {
        println!("FFmpeg is already installed! 🎉");
    }
    return true;
}
