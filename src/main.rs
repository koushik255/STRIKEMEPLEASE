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
use std::sync::Arc;
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

    let output = output_dir.to_string();
    tokio::spawn(async move {
        println!("Starting conversion...");
        if let Err(e) = convert_to_dash(&input, &output) {
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

fn strip_sub_mkv___(input: String, output: String) -> String {
    let tmp_srt = "/tmp/temp.srt";
    println!("Extracting to temp SRT…");
    println!("INPUT {}", input);

    let step1 = Command::new("ffmpeg")
        .args(["-y", "-i", &input, "-map", "0:4", "-c:s", "srt", tmp_srt])
        .output()
        .expect("failed ffmpeg step1");
    if !step1.status.success() {
        eprintln!("Step1 failed: {}", String::from_utf8_lossy(&step1.stderr));
    }

    println!("Converting temp SRT → VTT…");
    let step2 = Command::new("ffmpeg")
        .args(["-y", "-i", tmp_srt, "-c:s", "webvtt", &output])
        .output()
        .expect("failed ffmpeg step2");

    if step2.status.success() {
        println!("✅ Subtitle extraction successful: {output}");
    } else {
        eprintln!(
            "❌ Conversion failed: {}",
            String::from_utf8_lossy(&step2.stderr)
        );
    }
    output
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

fn convert_to_dash(input: &str, output_dir: &str) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    let output_path = format!("{}/manifest.mpd", output_dir);

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
            "-seg_duration",
            "4",
            "-use_template",
            "1",
            "-use_timeline",
            "1",
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

// problem it doesnt work when you skip around
