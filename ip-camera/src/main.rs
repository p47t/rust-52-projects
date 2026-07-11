//! Cross-platform IP Camera and Video Processing Server in Rust.
//! Uses OpenCV 5, Axum (v0.7), and Tokio for an asynchronous, high-performance streaming architecture.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::{routing::get, Router};
use tokio::sync::watch;
use tracing::{error, info};

use opencv::{
    core::{Mat, Point, Scalar},
    imgcodecs, imgproc,
    prelude::*,
};

mod camera;
mod handlers;
mod state;
mod template;

use handlers::{index_handler, stream_handler};
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing framework for logging.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting Rust IP Camera and video processing server...");

    // Create a default initial frame (a nice blue-gray blank screen) to avoid crashes.
    let mut initial_mat = Mat::new_rows_cols_with_default(
        480,
        640,
        opencv::core::CV_8UC3,
        Scalar::new(40.0, 30.0, 30.0, 0.0),
    )?;
    imgproc::put_text(
        &mut initial_mat,
        "Initializing camera feed...",
        Point::new(180, 240),
        imgproc::FONT_HERSHEY_SIMPLEX,
        0.8,
        Scalar::new(255.0, 255.0, 255.0, 0.0),
        2,
        imgproc::LINE_AA,
        false,
    )?;

    let mut initial_jpeg = opencv::core::Vector::<u8>::new();
    let params = opencv::core::Vector::<i32>::new();
    imgcodecs::imencode(".jpg", &initial_mat, &mut initial_jpeg, &params)?;

    // Shared thread-safe storage for the latest encoded frame.
    let frame_state = Arc::new(Mutex::new(initial_jpeg.to_vec()));
    let frame_state_clone = Arc::clone(&frame_state);

    // Watch channel to notify web clients of new frames.
    let (watch_tx, watch_rx) = watch::channel(0u64);

    // Spawn the hardware video capture loop inside an asynchronous blocking task
    // to isolate raw hardware operations from the web server thread.
    let camera_index = 0; // Default camera ID
    tokio::task::spawn_blocking(move || {
        if let Err(e) = camera::run_capture_loop(camera_index, frame_state_clone, watch_tx) {
            error!("Capture loop exited with error: {:?}", e);
        }
    });

    // Build the web server routes.
    let app_state = AppState {
        frame_state,
        watch_rx,
    };

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/stream", get(stream_handler))
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    info!("Web server running on http://localhost:8080");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
