use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;
use tokio::fs::File;
use tokio::net::TcpListener;
use tokio_util::io::ReaderStream;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

#[derive(Clone)]
struct AppState {
    input: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Convert your video once
    //
    let output_dir = "/tmp/dash";
    std::fs::create_dir_all(output_dir)?;

    let input =
        "/home/koushikk/Downloads/SHOWS/Friren/S01E01-The Journey's End [18D1CE8D].mkv".to_string();
    let (duration, resolution) = get_video_info(input.as_str())?;
    println!(
        "video duration {:.2}, seconds, Resoltuion :{}",
        duration, resolution
    );

    let output = output_dir.to_string();
    tokio::spawn(async move {
        println!("Starting conversion...");
        if let Err(e) = convert_to_dash(&input, &output, "0") {
            eprintln!("Conversion failed: {}", e);
        } else {
            println!("Conversion complete!");
        }
    });

    let state = AppState {
        input: "/home/koushikk/Downloads/SHOWS/Friren/S01E01-The Journey's End [18D1CE8D].mkv"
            .to_string(),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .nest_service("/dash", ServeDir::new(output_dir))
        .route("/subs.vtt", get(send_sub))
        .with_state(state)
        .layer(cors);

    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    println!("Streaming at http://localhost:8080/hls/master.m3u8");

    axum::serve(listener, app).await?;
    Ok(())
}

fn strip_sub_mkv(input: String, outpath: String) -> String {
    println!("extracing subtitles from {}", input);
    let output = Command::new("ffmpeg")
        .args([
            "-y", "-i", &input, "-map", "0:4", "-c:s", "webvtt", &outpath,
        ])
        .output()
        .expect("failed to execute ffmpeg");
    if output.status.success() {
        println!("Subtitle extraction successful!");
        println!("created the file biggets bruh");
    } else {
        // Print the error if it fails (ffmpeg writes errors to stderr)
        let error_message = String::from_utf8_lossy(&output.stderr);
        eprintln!("Error: {}", error_message);
    }
    outpath
}

fn convert_to_hls(input: &str, output_dir: &str) -> Result<()> {
    let output_path = format!("{}/master.m3u8", output_dir);
    let status = Command::new("ffmpeg")
        .args(&[
            "-i",
            input,
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-profile:v",
            "high",
            "-pix_fmt",
            "yuv420p",
            "-preset",
            "medium",
            "-crf",
            "23",
            "-sn",
            "-hls_time",
            "4",
            "-hls_playlist_type",
            "vod",
            "-hls_segment_filename",
            &format!("{}/segment_%03d.ts", output_dir),
            "-f",
            "hls",
            &output_path,
        ])
        .status()?;
    if !status.success() {
        return Err(anyhow::anyhow!("FFmpeg process failed"));
    }
    Ok(())
}

fn convert_to_dash(input: &str, output_dir: &str, duration: &str) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    let output_path = format!("{}/manifest.mpd", output_dir);

    let start_time = duration;
    let status = Command::new("ffmpeg")
        .args(&[
            "-ss",
            &start_time.to_string(),
            "-i",
            input,
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-profile:v",
            "high",
            "-pix_fmt",
            "yuv420p",
            "-preset",
            "ultrafast",
            "-tune",
            "zerolatency",
            "-crf",
            "28",
            "-sn",
            "-keyint_min",
            "72",
            "-g",
            "72",
            "-sc_threshold",
            "0",
            "-force_key_frames",
            "expr:gte(t,n_forced*3)",
            //dash options
            "-seg_duration",
            "6",
            "-use_template",
            "1",
            "-use_timeline",
            "1",
            "-adaptation_sets",
            "id=0,streams=v id=1,streams=a",
            "-avoid_negative_ts",
            "make_zero",
            "-f",
            "dash",
            &output_path,
        ])
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("FFmpeg process failed"));
    }
    Ok(())
}
#[derive(Deserialize)]
struct SubQuery {
    path: Option<String>,
}

async fn send_sub(State(state): State<AppState>) -> impl IntoResponse {
    println!("sending subs");
    println!("SENDING SUBS BLUD");

    let vtt_path = state.input.replace(".mkv", ".vtt");
    let vtt_file = PathBuf::from(&vtt_path);

    if vtt_file.exists() {
        println!("this means that {} Exists?", vtt_file.display());
    }
    if !vtt_file.exists() {
        println!("subtitles not found");
        let ahwda = strip_sub_mkv(state.input.clone(), vtt_path.clone());
        println!(
            "oh wait its not stripping because its found the file? {}",
            ahwda
        );
        println!("subtitles stripped file made!");
        return (StatusCode::NOT_FOUND, "subtitle not found").into_response();
    }
    println!("after check");
    match File::open(&vtt_file).await {
        Ok(file) => {
            println!("creaing check");
            let stream = ReaderStream::new(file);
            let mut res = Response::new(Body::from_stream(stream));
            res.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_static("text/vtt; charset=utf-8"),
            );
            res
        }
        Err(err) => {
            eprintln!("open error: {}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, "cannot open subtitle").into_response()
        }
    }
}

fn get_video_info(input: &str) -> Result<(f64, String)> {
    let output = Command::new("ffprobe")
        .args(&[
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            input,
        ])
        .output()?;

    let info: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    let duration: f64 = info["format"]["duration"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No duration found"))?
        .parse()?;

    // geting dimension
    let video_stream = info["streams"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["codec_type"] == "video")
        .ok_or_else(|| anyhow::anyhow!("No video stream found"))?;

    let width = video_stream["width"].as_u64().unwrap_or(1920);
    let height = video_stream["height"].as_u64().unwrap_or(1080);
    let resolution = format!("{}x{}", width, height);

    Ok((duration, resolution))
}

// problem it doesnt work when you skip around
