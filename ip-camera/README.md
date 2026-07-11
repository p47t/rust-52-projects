# Cross-Platform Rust IP Camera & Video Processing Server

A high-performance, production-ready IP Camera and real-time video processing server written in Rust. It utilizes OpenCV 5 for image capture and computer vision processing, Axum (v0.7) and Tokio for an asynchronous web server, and streams Motion JPEG (MJPEG) directly to standard web browsers via HTTP using a custom multipart boundary format.

It is designed to build and run seamlessly on both **Windows (MSVC)** and **Linux (specifically Raspberry Pi 4 running Raspberry Pi OS)**.

---

## Key Features

1. **Dedicated Multithreaded Capture Loop**: Open-hardware frame capture is isolated inside a `tokio::task::spawn_blocking` loop. This keeps the Axum async event loop completely non-blocking and highly responsive.
2. **OpenCV 5 Real-Time CV Pipeline**: Inside the hardware loop, the frame is processed using the OpenCV `imgproc` module to overlay:
   - A live smoothed FPS counter (calculated via an Exponential Moving Average).
   - A high-precision live timestamp.
   - An interactive HUD target bounding box in the center, with a sweeping red scanner laser and a live Gaussian Blur filter applied *only* inside the Region of Interest (ROI).
3. **On-Task JPEG Compression**: Frame matrices (`cv::Mat`) are compressed to JPEG (`imgcodecs::imencode`) directly on the background worker thread. The web server threads only handle broadcasting raw bytes, ensuring minimal CPU usage per web client.
4. **Subscribers Notification Watch Stream**: Multiple web client streams are served using a `tokio::sync::watch` channel. Streaming tasks wake up asynchronously only when a new frame is encoded, preventing busy polling or thread starvation.
5. **Modern Web UI Dashboard**: A responsive, glassmorphic dark-mode dashboard (with Google Fonts typography, animations, play/pause controls, snapshot capability, and fullscreen toggle) is served directly on the root endpoint.
6. **Resilient Offline Fallback**: If no camera device is connected (such as in headless VMs or CI environments), the server automatically falls back to an animated, high-performance mock camera generator showing a radar scope with concentric target grids.

---

## Project Structure

* **`Cargo.toml`**: Configures all dependencies, crate features, and compiler options.
* **`build.bat`**: Windows wrapper script that configures local linker, library paths, and runtime DLL paths for compiling locally.
* **`src/main.rs`**: Core entry point containing the camera driver loop, simulated feed generator, CV processing pipeline, and Axum MJPEG streaming server.
* **`opencv-rust-patch/`**: Local crate patch adapting bindings specifically for OpenCV 5 compatibility.

---

## Cross-Platform Compilation Guide

### 1. Windows (MSVC) Build Setup

To compile on Windows, you must have OpenCV 5 and LLVM/Clang installed (needed by the `clang-sys` crate to parse OpenCV C++ headers and generate Rust bindings).

#### Prerequisites:
1. **Visual Studio 2022 (with C++ Desktop Development)**: Standard MSVC compiler tools.
2. **LLVM/Clang**: Download the LLVM Windows installer (e.g., LLVM 16.x or newer) and ensure `libclang.dll` is in your environment `PATH` or set `LIBCLANG_PATH`.
3. **OpenCV 5.x**: Install or compile OpenCV 5.

#### Environment Configuration:
The project includes a `build.bat` file that automates setting the environment variables for cargo compilation. It sets:
* `OPENCV_LINK_PATHS`: Path to OpenCV `.lib` binary directories (e.g., `D:\code\opencv\build\install\x64\vc17\lib`).
* `OPENCV_INCLUDE_PATHS`: Path to OpenCV header files (e.g., `D:\code\opencv\build\install\include`).
* `OPENCV_LINK_LIBS`: List of OpenCV libraries to link (e.g., `opencv_core510`, `opencv_imgproc510`, etc.).
* `PATH`: Prepends the directory containing OpenCV `.dll` runtime files so the compiled executable can find them (e.g., `D:\code\opencv\build\install\x64\vc17\bin`).

#### Windows Compilation:
Simply use `build.bat` instead of direct `cargo` calls:
```cmd
# Run compiler check
.\build.bat check

# Build release binary
.\build.bat build --release

# Run the server
.\build.bat run --release
```

---

### 2. Linux & Raspberry Pi 4 (ARM64) Build Setup

On Linux (Raspberry Pi OS, Debian, or Ubuntu), OpenCV compilation is simplified by leveraging `pkg-config` and system libraries.

#### Prerequisites:
Install OpenCV 5 dev packages, `pkg-config`, and `clang` (required for code generation) via your system packet manager:

```bash
# Update repositories
sudo apt update

# Install build dependencies, clang, and pkg-config
sudo apt install -y build-essential clang libclang-dev pkg-config

# Install OpenCV 5 (if packaged) or compile OpenCV 5 from source.
# Ensure that OpenCV dev libraries are registered with pkg-config.
# You can check this by running:
pkg-config --modversion opencv4 # (Or opencv5 if registered under that name)
```

#### Compile from Source (If OpenCV 5 package is not available):
If compiling OpenCV 5 from source, run:
```bash
git clone https://github.com/opencv/opencv.git --depth=1 -b 5.x
cd opencv && mkdir build && cd build
cmake -D CMAKE_BUILD_TYPE=RELEASE -D CMAKE_INSTALL_PREFIX=/usr/local ..
make -j$(nproc)
sudo make install
sudo ldconfig
```

#### Environment Configuration:
The `opencv` crate build script uses `pkg-config` by default on Unix platforms.

If your OpenCV installation is in `/usr/local` (default for manual builds), ensure `pkg-config` can find it by exporting `PKG_CONFIG_PATH`:
```bash
export PKG_CONFIG_PATH=/usr/local/lib/pkgconfig:$PKG_CONFIG_PATH
export LD_LIBRARY_PATH=/usr/local/lib:$LD_LIBRARY_PATH
```

If you installed Clang, set the path to `libclang.so`:
```bash
export LIBCLANG_PATH=/usr/lib/llvm-14/lib # Adjust path based on installed version
```

#### Linux Compilation:
From inside the `ip-camera` directory, run standard cargo commands:
```bash
# Check compilation
cargo check

# Build production binary
cargo build --release

# Run the server
cargo run --release
```

---

## Streaming Architecture Deep Dive

```mermaid
flowchart TD
    subgraph Background Thread [spawn_blocking Task]
        A[VideoCapture / VideoSource] -->|Raw Frame cv::Mat| B[CV imgproc Pipeline]
        B -->|Overlay FPS, Timestamp, ROI| C[Sub-Matrix ROI Blurring]
        C -->|Overlay Bounding Box & Laser Line| D[imencode JPEG Compression]
        D -->|JPEG Bytes| E[Update shared Mutex<Vec<u8>>]
        E -->|Increment Frame ID| F[Notify watch::Sender]
    end

    subgraph Asynchronous Web Threads [Axum Server]
        G[HTTP /stream Client] -->|Subscribe| H[watch::Receiver]
        H -->|Awake on Notification| I[Read latest JPEG bytes]
        I -->|Format multipart/x-mixed-replace| J[Yield boundary chunk]
        J -->|Send to TCP socket| G
    end
```

The server divides work between:
* **Capture Loop**: Runs sequentially at camera's capture frequency. It does all heavy image matrix processing and JPEG encoding on a dedicated OS thread.
* **Axum Server**: Handles client connections asynchronously. Using `stream::unfold` coupled with `watch::Receiver`, each stream request remains asleep (yielding no execution overhead) until the capture loop notifies that a new frame is ready. Multiple concurrent browsers can stream without thread-blocking or latency penalty.
