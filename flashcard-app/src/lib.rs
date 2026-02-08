mod app;
mod card;
mod error;
mod sample;
mod sm2;
mod storage;

slint::include_modules!();

pub fn run() {
    app::run();
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: slint::android::AndroidApp) {
    slint::android::init(app).unwrap();
    run();
}
