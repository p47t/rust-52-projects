use anyhow::{anyhow, Context, Result};
use clap::{Parser, ValueEnum};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossterm::style::Stylize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

mod engine;
mod preprocessor;
mod detector;
mod vad;
mod speech;

use engine::GemmaVoiceEngine;
use preprocessor::AudioPreprocessor;
use detector::WakewordDetector;
use vad::VadRecorder;
use wakeword_engine::{convert_channels_to_mono, SpeechUnlockType};


#[derive(ValueEnum, Clone, Copy, Debug, PartialEq)]
enum WakewordType {
    Alexa,
    Mycroft,
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Local Voice Assistant (Project #52 capstone)", long_about = None)]
struct Args {
    /// Path to the GGUF model file
    #[arg(short, long, default_value = "../llm-local-studio-4/models/huggingface/lmstudio-community/gemma-4-E4B-it-GGUF/gemma-4-E4B-it-Q4_K_M.gguf")]
    model: PathBuf,

    /// Path to the mmproj GGUF file
    #[arg(short, long, default_value = "../llm-local-studio-4/models/huggingface/lmstudio-community/gemma-4-E4B-it-GGUF/mmproj-gemma-4-E4B-it-BF16.gguf")]
    mmproj: PathBuf,

    /// Wake word model to use
    #[arg(short, long, value_enum, default_value_t = WakewordType::Alexa)]
    wakeword: WakewordType,

    /// Confidence threshold for wakeword detection (0.0 to 1.0)
    #[arg(short, long, default_value_t = 0.5)]
    threshold: f32,

    /// Disable WebRTC APM preprocessing
    #[arg(long)]
    no_apm: bool,

    /// RMS energy silence threshold for VAD
    #[arg(short = 'v', long, default_value_t = 0.003)]
    vad_threshold: f32,

    /// Maximum recording duration in seconds
    #[arg(short = 'd', long, default_value_t = 8.0)]
    max_duration: f32,

    /// Seconds of continuous silence before ending recording
    #[arg(short = 's', long, default_value_t = 1.5)]
    silence_duration: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Listening,
    Recording,
    Processing,
}

fn print_state_banner(state: State) {
    match state {
        State::Listening => {
            println!(
                "\n{} {}",
                "[LISTEN]".bold().green(),
                "Listening for wake word... (Speak to trigger)".italic().white()
            );
        }
        State::Recording => {
            println!(
                "\n{} {}",
                "[RECORD]".bold().red(),
                "Listening to user query... (Stop talking to finalize)".white()
            );
        }
        State::Processing => {
            println!(
                "\n{} {}",
                "[THINK]".bold().blue(),
                "Evaluating audio query with Gemma-4...".yellow()
            );
        }
    }
}

fn save_wav(path: &Path, samples: &[f32], sample_rate: u32) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &sample in samples {
        let s = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
        writer.write_sample(s)?;
    }
    writer.finalize()?;
    Ok(())
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = Args::parse();

    println!("{}", "=============================================".cyan());
    println!("{}", "      PROJECT 52: LOCAL VOICE ASSISTANT      ".bold().cyan());
    println!("{}", "=============================================".cyan());
    println!("Wakeword Model: {:?}", args.wakeword);
    println!("Wakeword Threshold: {}", args.threshold);
    println!("VAD Threshold: {}", args.vad_threshold);
    println!("Silence Duration: {}s", args.silence_duration);
    println!("Max Recording Duration: {}s", args.max_duration);
    println!("Model Path: {}", args.model.display());
    println!("Projector Path: {}", args.mmproj.display());
    println!("{}", "=============================================\n".cyan());

    if !args.model.exists() {
        return Err(anyhow!(
            "Model file not found at: {}\nPlease verify the path or run llm-local-studio-4 first.",
            args.model.display()
        ));
    }
    if !args.mmproj.exists() {
        return Err(anyhow!(
            "Multimodal projector not found at: {}\nPlease verify the path.",
            args.mmproj.display()
        ));
    }

    // Step 1: Initialize Gemma Multimodal Audio Engine
    log::info!("Initializing Gemma-4 Multimodal Audio Engine (with GPU offload)...");
    let mut engine = GemmaVoiceEngine::load(&args.model, &args.mmproj, 4096)
        .context("Failed to load Gemma voice engine")?;
    log::info!("Engine successfully loaded.");

    // Step 2: Initialize openWakeWord Model
    log::info!("Initializing openWakeWord model...");
    let model_type = match args.wakeword {
        WakewordType::Alexa => SpeechUnlockType::OpenWakeWordAlexa,
        WakewordType::Mycroft => SpeechUnlockType::OpenWakeWordHeyMycroft,
    };
    let mut detector = WakewordDetector::new(model_type, args.threshold)?;
    log::info!("Wakeword model initialized.");

    // Step 3: Configure Audio Input device (Microphone)
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("No input device found"))?;

    let device_name = device.name().unwrap_or_else(|_| "Unknown Device".to_string());
    log::info!("Using input device: {}", device_name);

    let config = device
        .default_input_config()
        .map_err(|e| anyhow!("Failed to get default input config: {:?}", e))?;

    let input_sample_rate = config.sample_rate().0;
    let input_channels = config.channels();
    log::info!(
        "Microphone input format: sample_rate={} Hz, channels={}",
        input_sample_rate, input_channels
    );

    // Step 4: Initialize Audio Preprocessor
    let mut preprocessor = AudioPreprocessor::new(input_sample_rate, !args.no_apm)?;
    
    // Step 5: Initialize VAD Recorder
    let mut vad_recorder = VadRecorder::new(args.vad_threshold, args.silence_duration, args.max_duration);

    // Shared buffer between audio thread and processing loop
    let input_buffer = Arc::new(Mutex::new(Vec::new()));
    let capture_buffer = input_buffer.clone();

    // Stream error callback
    let err_fn = |err| log::error!("An error occurred on stream: {:?}", err);

    log::info!("Starting microphone stream...");
    
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let mono_data = convert_channels_to_mono(data, input_channels as usize);
                if let Ok(mut buf) = capture_buffer.lock() {
                    buf.extend_from_slice(&mono_data);
                }
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                let f32_data: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                let mono_data = convert_channels_to_mono(&f32_data, input_channels as usize);
                if let Ok(mut buf) = capture_buffer.lock() {
                    buf.extend_from_slice(&mono_data);
                }
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::U16 => device.build_input_stream(
            &config.into(),
            move |data: &[u16], _: &cpal::InputCallbackInfo| {
                let f32_data: Vec<f32> = data.iter().map(|&s| (s as f32 - 32768.0) / 32768.0).collect();
                let mono_data = convert_channels_to_mono(&f32_data, input_channels as usize);
                if let Ok(mut buf) = capture_buffer.lock() {
                    buf.extend_from_slice(&mono_data);
                }
            },
            err_fn,
            None,
        )?,
        _ => return Err(anyhow!("Unsupported sample format")),
    };

    stream.play()?;
    log::info!("Microphone stream is active.");

    // State machine variables
    let mut state = State::Listening;
    print_state_banner(state);

    loop {
        // Fetch new raw samples from the shared capture buffer
        let new_samples = {
            if let Ok(mut buf) = input_buffer.lock() {
                let samples = buf.clone();
                buf.clear();
                samples
            } else {
                Vec::new()
            }
        };

        // Feed to preprocessor: channel mixes (if not done in stream callback), resamples to 16kHz, NS + AGC
        let processed_samples = preprocessor.feed_and_process(&new_samples, 1)?;

        if !processed_samples.is_empty() {
            match state {
                State::Listening => {
                    // Feed preprocessed 16kHz mono samples to openWakeWord
                    if detector.feed_and_detect(&processed_samples) {
                        speech::play_notification_beep(880.0, 150); // Beep high A
                        state = State::Recording;
                        print_state_banner(state);
                        vad_recorder.reset();
                        detector.reset();
                    }
                }
                State::Recording => {
                    let mut finished_recording = false;

                    // Process frame-by-frame (VAD/RMS operates on 10ms / 160-sample chunks)
                    for frame in processed_samples.chunks_exact(160) {
                        let (is_finalized, rms) = vad_recorder.process_frame(frame);
                        
                        // Print volume meter to terminal for visual feedback
                        let meter_len = (rms * 250.0).clamp(0.0, 30.0) as usize;
                        let meter = "█".repeat(meter_len) + &" ".repeat(30 - meter_len);
                        print!(
                            "\r[RECORDING] RMS: {:.5} [{}]",
                            rms,
                            meter.red()
                        );
                        std::io::stdout().flush().unwrap();

                        if is_finalized {
                            finished_recording = true;
                            break;
                        }
                    }

                    if finished_recording {
                        println!("\nRecording finished.");
                        speech::play_notification_beep(440.0, 150); // Beep low A
                        state = State::Processing;
                        print_state_banner(state);

                        // Save query WAV
                        let temp_wav_path = Path::new("temp_query.wav");
                        save_wav(temp_wav_path, vad_recorder.samples(), 16000)?;

                        println!("Prompting Gemma-4 multimodal projector...");
                        let response_text = Arc::new(Mutex::new(String::new()));
                        let response_text_clone = response_text.clone();

                        // Streaming inference callback
                        let res = engine.run_multimodal(
                            "Respond to the user's spoken instruction. Be concise, polite, and helpful.",
                            temp_wav_path,
                            256,
                            None,
                            move |piece| {
                                print!("{}", piece);
                                std::io::stdout().flush().unwrap();
                                response_text_clone.lock().unwrap().push_str(piece);
                            }
                        );

                        println!(); // Trailing newline
                        let _ = std::fs::remove_file(temp_wav_path);

                        match res {
                            Ok(_) => {
                                println!("\n[SPEAKING]");
                                let text_to_speak = response_text.lock().unwrap().clone();
                                println!("TTS: {}", text_to_speak);
                                speech::speak(&text_to_speak);
                            }
                            Err(e) => {
                                log::error!("Inference error: {:?}", e);
                            }
                        }

                        // Transition back to Listening and reset state
                        state = State::Listening;
                        print_state_banner(state);

                        // Clear buffers to avoid feedback or stale voice inputs
                        if let Ok(mut buf) = input_buffer.lock() {
                            buf.clear();
                        }
                        preprocessor.reset();
                        detector.reset();
                        vad_recorder.reset();
                    }
                }
                State::Processing => {
                    // Handled synchronously in Recording transition
                }
            }
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}
