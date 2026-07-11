use axum::{
    body::{Body, Bytes},
    extract::State,
    http::header::{HeaderMap, HeaderValue, CONTENT_TYPE},
    response::{Html, IntoResponse},
};
use futures_util::stream;

use crate::state::AppState;
use crate::template::HTML_TEMPLATE;

/// The root handler that serves the responsive telemetry dashboard.
pub async fn index_handler() -> impl IntoResponse {
    let platform = if cfg!(target_os = "windows") {
        "Windows (MSVC)"
    } else {
        "Linux (Raspberry Pi / ARM64)"
    };
    let html = HTML_TEMPLATE.replace("_OS_PLATFORM_", platform);
    Html(html)
}

/// The streaming route handler that generates a multipart MJPEG response.
pub async fn stream_handler(State(state): State<AppState>) -> impl IntoResponse {
    let rx = state.watch_rx.clone();
    let frame_state = state.frame_state.clone();

    // Build a stream using `futures_util::stream::unfold`.
    // It remains asleep on `rx.changed()` and wakes up only when a new frame is ready.
    let stream = stream::unfold((rx, frame_state), move |(mut rx, frame_state)| async move {
        if rx.changed().await.is_err() {
            // Sender dropped, end stream.
            return None;
        }

        // Retrieve the latest JPEG bytes from the shared memory.
        let jpeg_bytes = {
            let lock = frame_state.lock().unwrap();
            lock.clone()
        };

        if jpeg_bytes.is_empty() {
            // Return empty chunk to keep stream alive.
            return Some((Ok::<Bytes, std::io::Error>(Bytes::new()), (rx, frame_state)));
        }

        // Construct the boundary multipart packet.
        let mut body_bytes = Vec::new();
        let header = format!(
            "--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
            jpeg_bytes.len()
        );
        body_bytes.extend_from_slice(header.as_bytes());
        body_bytes.extend_from_slice(&jpeg_bytes);
        body_bytes.extend_from_slice(b"\r\n");

        Some((
            Ok::<Bytes, std::io::Error>(Bytes::from(body_bytes)),
            (rx, frame_state),
        ))
    });

    let body = Body::from_stream(stream);

    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("multipart/x-mixed-replace; boundary=frame"),
    );

    (headers, body)
}
