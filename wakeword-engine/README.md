# Wakeword Engine

A high-performance, real-time wake word detection engine built in Rust. It utilizes the pure-Rust WebRTC Audio Processing Module (APM) for audio preprocessing (noise suppression and automatic gain control) and the openWakeWord ONNX models for wake word classification.

## Features

- **WebRTC APM Preprocessing**: Built-in noise suppression (NS) and Automatic Gain Control (AGC2) via [sonora](https://crates.io/crates/sonora) to clean audio before inference.
- **Embedded openWakeWord ONNX Models**: Runs wake word classification natively in Rust via [oww-rs](https://crates.io/crates/oww-rs) (tract-onnx), embedding models (`Alexa` and `HeyMycroft`) into the binary.
- **Microphone Capture**: Captures real-time audio streams using [cpal](https://crates.io/crates/cpal) and resamples inputs to 16 kHz.
- **File Processing**: Batch process `.wav` audio files to extract timestamps of wake word triggers.
- **FAR and FRR Benchmarking**: A built-in subcommand to evaluate False Acceptance Rate (FAR) and False Rejection Rate (FRR) over directories of audio samples.

## Running the Engine

Always build and run in `--release` mode, as ONNX inference in debug mode is CPU-intensive.

```bash
# Compile release build
cargo build --release
```

### 1. Live Microphone Mode (Default)
Capture and listen from the default microphone:
```bash
./target/release/wakeword-engine.exe --wakeword alexa --threshold 0.5
```

### 2. File Mode
Analyze a WAV file for wake words:
```bash
./target/release/wakeword-engine.exe --input path/to/recording.wav
```
To save the preprocessed audio (after WebRTC NS & AGC) to verify APM:
```bash
./target/release/wakeword-engine.exe --input recording.wav --output-debug-wav clean.wav
```

### 3. Benchmarking Subcommand (FAR & FRR)
Evaluate your detection accuracy and false trigger rates over datasets:
```bash
./target/release/wakeword-engine.exe benchmark --positive-dir path/to/positives/ --negative-dir path/to/negatives/
```
- **FRR (False Rejection Rate)** is computed as the percentage of positive files (containing the wake word) where the engine failed to fire.
- **FAR (False Acceptance Rate)** is computed as the percentage of negative files (background noise, podcasts) where the engine fired.
- **False Alarms per Hour** is computed by summing the durations of all negative audio files and dividing the total false triggers by the total hours of negative audio.

## Running Integration Tests

We have written integration tests under `tests/far_frr.rs` that synthesize silence, 440Hz sine waves, and white noise, verifying that the False Acceptance Rate (FAR) on non-speech sounds remains at exactly 0.0%:

```bash
cargo test
```
