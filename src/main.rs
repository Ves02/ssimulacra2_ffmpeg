use bytemuck;
use clap::{arg, command, value_parser};
use ffmpeg_sidecar::command::*;
use ssimulacra2::{compute_frame_ssimulacra2, LinearRgb};
use std::io::{self, Write};
use std::process::Command;
use std::collections::HashMap;
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
    let source_iter = new_iterator(source);

    let distorted = matches.get_one::<String>("distorted").unwrap();
    let distorted_iter = new_iterator(distorted);

    let mut s_preprocessed = preprocess(source_iter.filter_frames());
    let mut d_preprocessed = preprocess(distorted_iter.filter_frames());
    

    while let (Some(s_frame), Some(d_frame)) = (s_preprocessed.next(), d_preprocessed.next()) {
        let score = compute_frame_ssimulacra2(s_frame, d_frame).unwrap();
        println!("{}", score);
    }

    return Ok(());
}

fn new_iterator<S: AsRef<str>>(path: &S) -> ffmpeg_sidecar::iter::FfmpegIterator{
    let mut s_metadata = ffprobe_get_color_metadata(path).unwrap();

    if s_metadata.get("color_space") == Some(&"unknown".to_string()) {
        println!("color space not found, defaulting to bt709");
        s_metadata.insert("color_space".to_string(), "bt709".to_string());
    }
    if s_metadata.get("color_primaries") == Some(&"unknown".to_string()) {
        println!("color primaries not found, defaulting to bt709");
        s_metadata.insert("color_primaries".to_string(), "bt709".to_string());
    }
    if s_metadata.get("color_transfer") == Some(&"unknown".to_string()) {
        println!("color space not found, defaulting to bt709");
        s_metadata.insert("color_transfer".to_string(), "bt709".to_string());
    }

    let zscale = format!("zscale=primariesin={}:transferin={}:matrixin={}:transfer=linear", s_metadata.get("color_primaries").unwrap(), s_metadata.get("color_transfer").unwrap(), s_metadata.get("color_space").unwrap());
    println!("{:?}",zscale);

    // ffmpeg libswscale/output.c does not support rgbf32le, which is more ideal output format. We will have to convert to rgbf32le.
    let source_iter = FfmpegCommand::new()
        .input(path)
        .pix_fmt("gbrpf32le")
        .fps_mode("passthrough")
        .filter(zscale)
        .format("rawvideo")
        .pipe_stdout()
        // .print_command()
        .spawn().unwrap()
        .iter().unwrap();

    return source_iter
}


fn preprocess<I>(input_iter: I) -> impl Iterator<Item = LinearRgb>
where
    I: Iterator<Item = ffmpeg_sidecar::event::OutputVideoFrame>,
{
    input_iter
        .map(|raw_frame| {
            // convert pixel data
            let s_data = convert_bgrpf32le_to_rgbf32le(raw_frame.data);
            // build your LinearRgb, propagating width/height
            LinearRgb::new(
                s_data,
                raw_frame.width.try_into().unwrap(),
                raw_frame.height.try_into().unwrap(),
            )
            .unwrap()
        })
}

fn ffprobe_get_color_metadata<S: AsRef<str>>(file: S) -> Result<HashMap<String, String>, String> {
    // ffprobe -v error -select_streams v:0 -show_entries stream=width,height -of csv=p=0;
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-show_entries")
        .arg("stream=color_space,color_transfer,color_primaries")
        .arg("-of")
        .arg("csv=p=0")
        .arg(file.as_ref())
        .output()
        .map_err(|e| format!("Failed to execute ffprobe: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(format!("ffprobe error: {stderr}"));
    }

    let output = String::from_utf8(output.stdout)
        .map_err(|e| format!("Failed to convert ffprobe output to UTF-8: {}", e))?;

    let output = output.trim();
    if output.is_empty() {
        return Err("No resolution found.".into());
    }

    // Split the resolution by ',' and parse the components into integers
    let mut parts: Vec<&str> = output.split(",").collect();
    // println!("{:?}", parts);
    if parts.len() != 3 {
        return Err("Could not get 3 keys".into());
    }

    let mut metadata: HashMap<String, String> = HashMap::new();

    // for part in parts {
    //     if let Some((key, value)) = part.split_once('=') {
    //         metadata.insert(key.to_string(), value.to_string());
    //     }
    // }

    // color_space,color_transfer,color_primaries
    metadata.insert("color_space".into(), parts.pop().unwrap().into());
    metadata.insert("color_transfer".into(), parts.pop().unwrap().into());
    metadata.insert("color_primaries".into(), parts.pop().unwrap().into());

    Ok(metadata)
}

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
