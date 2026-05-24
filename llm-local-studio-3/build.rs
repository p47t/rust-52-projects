use std::process::Command;
use std::path::Path;

fn main() {
    let ui_dir = Path::new("ui");

    // Only run if the ui directory exists
    if ui_dir.exists() {
        // Tell cargo to re-run the build script if these files change
        println!("cargo:rerun-if-changed=ui/package.json");
        println!("cargo:rerun-if-changed=ui/package-lock.json");
        println!("cargo:rerun-if-changed=ui/index.html");
        println!("cargo:rerun-if-changed=ui/vite.config.js");
        // Also watch the src directory for changes
        println!("cargo:rerun-if-changed=ui/src");

        // Install npm dependencies
        let npm_cmd = if cfg!(windows) { "npm.cmd" } else { "npm" };

        let install_status = Command::new(npm_cmd)
            .current_dir(ui_dir)
            .args(["install"])
            .status()
            .expect("Failed to execute npm install. Is npm installed and in your PATH?");

        if !install_status.success() {
            panic!("npm install failed");
        }

        // Run Vite build
        let build_status = Command::new(npm_cmd)
            .current_dir(ui_dir)
            .args(["run", "build"])
            .status()
            .expect("Failed to execute npm run build.");

        if !build_status.success() {
            panic!("npm run build failed");
        }
    }
}
