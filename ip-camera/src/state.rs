use std::sync::{Arc, Mutex};
use tokio::sync::watch;

/// Shared state between the camera capture thread and the web server handlers.
#[derive(Clone)]
pub struct AppState {
    /// Mutex protecting the latest encoded JPEG frame bytes.
    pub frame_state: Arc<Mutex<Vec<u8>>>,
    /// Watch receiver that notifies when a new frame is captured.
    pub watch_rx: watch::Receiver<u64>,
}
