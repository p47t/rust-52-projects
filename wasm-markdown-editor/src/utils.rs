/// Set up panic hook for better error messages in the browser console
pub fn set_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Log a message to the browser console
#[allow(dead_code)]
pub fn log(msg: &str) {
    web_sys::console::log_1(&msg.into());
}
