use clap::{arg, command, value_parser};
use ssimulacra2::{compute_frame_ssimulacra2, LinearRgb};
use std::error::Error;
use std::io::Read;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use threadpool::ThreadPool;

struct Resolution {
    width: usize,
    height: usize,
}

impl Resolution {
    // Constructor for creating a Resolution object
    fn new(width: usize, height: usize) -> Self {
        Resolution { width, height }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let avail_threads = thread::available_parallelism().unwrap();
    let matches = command!()
        .arg(
            arg!([source] "Source video or image(s)")
                .required(true)
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            arg!([distorted] "Distorted video or image(s)")
                .required(true)
                .value_parser(value_parser!(PathBuf)),
        )
        .get_matches();

    let file1 = matches.get_one::<PathBuf>("source").unwrap();
    let mut imgs1 = ffmpeg_get_frames_bgrpf32le(&file1)?;

    let file2 = matches.get_one::<PathBuf>("distorted").unwrap();
    let mut imgs2 = ffmpeg_get_frames_bgrpf32le(&file2)?;

    let scores = Arc::new(Mutex::new(Vec::new()));

    let pool = ThreadPool::new(avail_threads.into());
    println!("\nComparision in progress...");
    while let (Some(img1), Some(img2)) = (imgs1.pop(), imgs2.pop()) {
        let scores = Arc::clone(&scores);
        pool.execute(move || {
            let result =
                compute_frame_ssimulacra2(img1, img2).expect("Failed to calculate ssimulacra2");
            println!("{}", result);
            let mut scores = scores.lock().unwrap();
            scores.push(result);
        });
    }

    pool.join();
    let scores = scores.lock().unwrap();
    calculate_scores(scores.clone());

    Ok(())
}

// Also serve to check if it is a image, video, or image sequences file(s)
// ffprobe will output only number of frames if a file is valid.
fn ffprobe_get_num_frames(file: &Path) -> Result<NonZeroUsize, Box<dyn Error>> {
    // ffprobe -v error -select_streams v:0 -count_packets -show_entries stream=nb_read_packets -of csv=p=0 input.mp4
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-count_packets")
        .arg("-show_entries")
        .arg("stream=nb_read_packets")
        .arg("-of")
        .arg("csv=p=0")
        .arg(file) // file variable
        .output()
        .map_err(|e| format!("Failed to execute ffprobe: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(format!("ffprobe error: {stderr}").into());
    }

    let output_str = String::from_utf8(output.stdout)
        .map_err(|e| format!("Failed to convert ffprobe output to UTF-8: {}", e))?;

    let output_str = output_str.trim();
    if output_str.is_empty() {
        return Err("No frame count found.".into());
    }

    let frame_count = output_str
        .parse::<NonZeroUsize>()
        .map_err(|e| format!("Failed to parse output as NonZero<usize>: {}", e))?;

    return Ok(frame_count);
}

fn ffprobe_get_resolution(file: &Path) -> Result<Resolution, String> {
    // ffprobe -v error -select_streams v:0 -show_entries stream=width,height -of csv=p=0;
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-show_entries")
        .arg("stream=width,height")
        .arg("-of")
        .arg("csv=p=0")
        .arg(file)
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
    let parts: Vec<&str> = output.split(",").collect();
    if parts.len() != 2 {
        return Err("Invalid resolution format.".into());
    }

    let width = parts[0]
        .parse::<usize>()
        .map_err(|e| format!("Failed to parse width: {}", e))?;
    let height = parts[1]
        .parse::<usize>()
        .map_err(|e| format!("Failed to parse height: {}", e))?;

    let resolution = Resolution::new(width, height);
    Ok(resolution)
}

// for some reason, you can't do rgbf32le even though it exist on https://ffmpeg.org/doxygen/trunk/pixfmt_8h_source.html
// gbrpf32le is the next best alternative, you need to swap green and red channel position.
// ffmpeg -i input_file -vf zscale=t=linear:npl=100 -pix_fmt gbrpf32le -f rawvideo -
fn ffmpeg_get_frames_bgrpf32le(file: &Path) -> Result<Vec<LinearRgb>, Box<dyn Error>> {
    // fix the colorspace, color_trc, and color_primaries where it's unknown, otherwise crash since ffmpeg need those info for zscale=t=linear
    let frame_count = ffprobe_get_num_frames(&file)?;
    let resolution = ffprobe_get_resolution(&file)?;

    let mut child = Command::new("ffmpeg")
        // .arg("-color_trc")
        // .arg("bt709")
        // .arg("-color_primaries")
        // .arg("bt709")
        // .arg("-colorspace")
        // .arg("rgb")
        .arg("-i")
        .arg(file)
        .arg("-vf")
        .arg("zscale=t=linear:npl=100")
        .arg("-pix_fmt")
        .arg("gbrpf32le")
        .arg("-f")
        .arg("rawvideo")
        // .arg("-threads")
        // .arg("16")
        .arg("-")
        .stdout(Stdio::piped())
        .spawn()?;

    let mut stdout = child.stdout.take().expect("Failed to capture stdout");

    // stdout.read_to_end require u8, cannot convert to f32
    // let mut temp_data: Vec<u8> = Vec::new();
    // stdout.read_to_end(&mut temp_data)?;

    let mut buffer = [0u8; 8192]; // 8 KB buffer
    let mut temp_data = Vec::new();
    while let Ok(n) = stdout.read(&mut buffer) {
        if n == 0 {
            break;
        }
        temp_data.extend_from_slice(&buffer[..n]);
    }

    // if temp_data.len() != (resolution.width * resolution.height * 3 * 4 * usize::from(frame_count))
    // {
    //     println!(
    //         "Total byte size should be width * dimension * 3 channel * 4 bytes * number of frames"
    //     );
    //     println!(
    //         "Calculated size: {}",
    //         resolution.width * resolution.height * 3 * 4 * usize::from(frame_count)
    //     );
    //     println!("temp data size: {}", temp_data.len());
    //     return Err("Error processing temp data".into());
    // }

    let data: Vec<[f32; 3]> = temp_data
        .chunks_exact(12) // 12 bytes per pixel (3 channels * 4 bytes for each f32)
        .map(|chunk| {
            // For each 12-byte chunk (GBR channels), extract the 3 f32 values
            let g = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            let b = f32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
            let r = f32::from_le_bytes([chunk[8], chunk[9], chunk[10], chunk[11]]);

            // Return the values as [Red, Green, Blue] (RGB order)
            [r, g, b] // Returning an array directly
        })
        .collect();

    // println!("data size: {}", data.len());

    let frame_size = resolution.width * resolution.height;

    // if data.len() != (resolution.width * resolution.height * usize::from(frame_count)) {
    //     println!("Data should have same size as width x height * number of frames");
    //     println!(
    //         "Calculated size: {}",
    //         resolution.width * resolution.height * usize::from(frame_count)
    //     );
    //     println!("Data size: {}", data.len());
    //     return Err("Error processing data!!".into());
    // }

    let mut frames = Vec::new();
    for i in 0..frame_count.into() {
        let start = i * frame_size;
        let end = start + frame_size;
        let frame = data[start..end].to_vec();

        let buffer = LinearRgb::new(frame, resolution.width, resolution.height)
            .expect("Failed to process data into LinearRGB");

        frames.push(buffer);
    }
    child.wait()?;
    return Ok(frames);
}

//
fn calculate_scores(mut scores: Vec<f64>) {
    scores.sort_by(|a, b| a.partial_cmp(b).unwrap());

    // Calculate Mean
    let sum: f64 = scores.iter().sum();
    let mean = sum / scores.len() as f64;

    // Calculate Median
    let median = if scores.len() % 2 == 0 {
        let mid = scores.len() / 2;
        (scores[mid - 1] + scores[mid]) / 2.0
    } else {
        scores[scores.len() / 2]
    };

    // Calculate Standard Deviation
    let variance = scores.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / scores.len() as f64;
    let std_dev = variance.sqrt();

    // Calculate 5th and 95th Percentiles
    let percentile = |p: f64| {
        let pos = p * (scores.len() as f64 - 1.0);
        let lower = pos.floor() as usize;
        let upper = pos.ceil() as usize;
        let weight = pos - lower as f64;
        scores[lower] * (1.0 - weight) + scores[upper] * weight
    };
    let p5 = percentile(0.05);
    let p95 = percentile(0.95);

    // Print Results
    println!("\nMean: {:.8}", mean);
    println!("Median: {:.8}", median);
    println!("Std Dev: {:.8}", std_dev);
    println!("5th Percentile: {:.8}", p5);
    println!("95th Percentile: {:.8}", p95);
}
