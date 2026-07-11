use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand, ValueEnum};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::{WavReader, WavSpec, WavWriter};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use wakeword_engine::{
    convert_channels_to_mono, get_processed_samples, init_apm,
    process_wav_file, resample_linear, OwwModel, SpeechUnlockType,
    OWW_MODEL_CHUNK_SIZE,
};

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq)]
enum WakewordType {
    Alexa,
    Mycroft,
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Wakeword engine using WebRTC APM and openWakeWord", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Wake word model to use
    #[arg(short, long, value_enum, default_value_t = WakewordType::Alexa)]
    wakeword: WakewordType,

    /// Confidence threshold for detection (0.0 to 1.0)
    #[arg(short, long, default_value_t = 0.5)]
    threshold: f32,

    /// Disable WebRTC APM preprocessing (raw audio will be used)
    #[arg(long)]
    no_apm: bool,

    /// Path to a WAV file to process instead of using the microphone
    #[arg(short, long)]
    input: Option<String>,

    /// Save the preprocessed audio to a WAV file (useful for debugging APM output)
    #[arg(short, long)]
    output_debug_wav: Option<String>,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    /// Run benchmark over directories of positive/negative samples to calculate FAR and FRR
    Benchmark {
        /// Directory containing positive samples (WAV files with wake word)
        #[arg(long)]
        positive_dir: Option<String>,

        /// Directory containing negative samples (WAV files without wake word)
        #[arg(long)]
        negative_dir: Option<String>,
    },
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = Args::parse();

    log::info!("Initializing openWakeWord model...");
    let model_type = match args.wakeword {
        WakewordType::Alexa => SpeechUnlockType::OpenWakeWordAlexa,
        WakewordType::Mycroft => SpeechUnlockType::OpenWakeWordHeyMycroft,
    };

    let oww_model = OwwModel::new(model_type, args.threshold)
        .map_err(|e| anyhow!("Failed to initialize openWakeWord model: {:?}", e))?;

    log::info!("Model initialized. Target wake word: {:?}", args.wakeword);

    match &args.command {
        Some(Commands::Benchmark { positive_dir, negative_dir }) => {
            run_benchmark(
                positive_dir.as_deref(),
                negative_dir.as_deref(),
                oww_model,
                &args,
            )?;
        }
        None => {
            if let Some(ref input_path) = args.input {
                run_file_mode(input_path, oww_model, &args)?;
            } else {
                run_mic_mode(oww_model, &args)?;
            }
        }
    }

    Ok(())
}

fn run_file_mode(input_path: &str, mut oww_model: OwwModel, args: &Args) -> Result<()> {
    log::info!("Processing file: {}", input_path);
    let processed_samples = get_processed_samples(input_path, !args.no_apm)?;

    // Optionally save debug WAV file
    if let Some(ref output_path) = args.output_debug_wav {
        log::info!("Saving preprocessed audio to {}", output_path);
        let write_spec = WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = WavWriter::create(output_path, write_spec)?;
        for &sample in &processed_samples {
            let s = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
            writer.write_sample(s)?;
        }
        writer.finalize()?;
    }

    // Run wake word detection
    log::info!("Running wake word inference...");
    let chunk_size = OWW_MODEL_CHUNK_SIZE; // 1280 samples
    let total_samples = processed_samples.len();
    let mut offset = 0;
    let mut detections = 0;

    while offset + chunk_size <= total_samples {
        let chunk = &processed_samples[offset..offset + chunk_size];
        let triggered = oww_model.detection(chunk.to_vec());
        if triggered.detected {
            let timestamp_sec = offset as f32 / 16000.0;
            println!(
                "[{:.2}s] 🚨 WAKEWORD DETECTED ({:?}) (prob={:.2})!",
                timestamp_sec, args.wakeword, triggered.probability
            );
            detections += 1;
        }
        offset += chunk_size;
    }

    log::info!("File processing completed. Total detections: {}", detections);
    Ok(())
}

fn run_mic_mode(mut oww_model: OwwModel, args: &Args) -> Result<()> {
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

    // Initialize WebRTC APM at 16000 Hz
    let mut apm = if !args.no_apm {
        log::info!("WebRTC APM enabled (Noise Suppression & AGC)");
        Some(init_apm(16000)?)
    } else {
        log::info!("WebRTC APM disabled");
        None
    };

    // Shared state between capture thread and processing loop
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

    println!("\n=== Wakeword Engine Listening ===");
    println!("Wake Word: {:?}", args.wakeword);
    println!("Threshold: {}", args.threshold);
    println!("Press Ctrl+C to exit...\n");

    // Processing buffers
    let mut mic_samples_accumulator = Vec::new();
    let mut apm_input_buffer = Vec::new();
    let mut oww_chunk_buffer = Vec::new();

    // Loop for processing
    loop {
        // Fetch new samples from shared buffer
        let new_samples = {
            if let Ok(mut buf) = input_buffer.lock() {
                let samples = buf.clone();
                buf.clear();
                samples
            } else {
                Vec::new()
            }
        };

        if !new_samples.is_empty() {
            mic_samples_accumulator.extend_from_slice(&new_samples);
        }

        let mic_samples_needed = (160.0 * (input_sample_rate as f32 / 16000.0)).ceil() as usize;

        if mic_samples_accumulator.len() >= mic_samples_needed {
            let chunk_to_resample = mic_samples_accumulator.drain(0..mic_samples_needed).collect::<Vec<_>>();
            let resampled_16k = resample_linear(&chunk_to_resample, input_sample_rate, 16000);
            apm_input_buffer.extend_from_slice(&resampled_16k);
        }

        // WebRTC APM works on 10ms (160 samples at 16kHz)
        let apm_frame_size = 160;
        while apm_input_buffer.len() >= apm_frame_size {
            let frame: Vec<f32> = apm_input_buffer.drain(0..apm_frame_size).collect();
            
            let processed_frame = if let Some(ref mut apm_engine) = apm {
                let mut dest = vec![0.0f32; apm_frame_size];
                apm_engine.process_capture_f32(&[&frame], &mut [&mut dest])
                    .map_err(|e| anyhow!("APM process error: {:?}", e))?;
                dest
            } else {
                frame
            };

            oww_chunk_buffer.extend_from_slice(&processed_frame);
        }

        // openWakeWord works on 1280 samples (80ms at 16kHz)
        let oww_chunk_size = OWW_MODEL_CHUNK_SIZE;
        while oww_chunk_buffer.len() >= oww_chunk_size {
            let chunk: Vec<f32> = oww_chunk_buffer.drain(0..oww_chunk_size).collect();
            let triggered = oww_model.detection(chunk);
            if triggered.detected {
                println!(
                    "🚨 [{}] WAKEWORD DETECTED ({:?}) (prob={:.2})!",
                    chrono::Local::now().format("%H:%M:%S"),
                    args.wakeword,
                    triggered.probability
                );
            }
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

fn run_benchmark(
    positive_dir: Option<&str>,
    negative_dir: Option<&str>,
    mut oww_model: OwwModel,
    args: &Args,
) -> Result<()> {
    println!("\n=== Starting FAR & FRR Benchmark ===");
    println!("Wake Word: {:?}", args.wakeword);
    println!("Threshold: {}", args.threshold);
    println!("WebRTC APM: {}\n", if args.no_apm { "Disabled" } else { "Enabled" });

    if positive_dir.is_none() && negative_dir.is_none() {
        return Err(anyhow!("Must specify at least one of --positive-dir or --negative-dir"));
    }

    if let Some(pos_dir) = positive_dir {
        println!("Evaluating Positive Samples (FRR)...");
        let mut tp = 0;
        let mut fr = 0;
        let entries = std::fs::read_dir(pos_dir)
            .map_err(|e| anyhow!("Failed to read positive directory: {}", e))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("wav") {
                let path_str = path.to_string_lossy();
                match process_wav_file(&path_str, &mut oww_model, !args.no_apm) {
                    Ok(detections) => {
                        if detections > 0 {
                            tp += 1;
                            println!("  [TP] {}: Detected {} times", path.file_name().unwrap().to_string_lossy(), detections);
                        } else {
                            fr += 1;
                            println!("  [FR] {}: Missed!", path.file_name().unwrap().to_string_lossy());
                        }
                    }
                    Err(e) => {
                        log::error!("Error processing {}: {:?}", path_str, e);
                    }
                }
            }
        }
        let total_pos = tp + fr;
        if total_pos > 0 {
            let frr = (fr as f32 / total_pos as f32) * 100.0;
            println!("\nPositive Results:");
            println!("  Total Positive Files: {}", total_pos);
            println!("  True Positives (TP):  {}", tp);
            println!("  False Rejections (FR): {}", fr);
            println!("  False Rejection Rate (FRR): {:.2}%\n", frr);
        } else {
            println!("No WAV files found in positive directory.\n");
        }
    }

    if let Some(neg_dir) = negative_dir {
        println!("Evaluating Negative Samples (FAR)...");
        let mut tn = 0;
        let mut fa = 0;
        let mut total_fa_triggers = 0;
        let mut total_duration_secs = 0.0;
        let entries = std::fs::read_dir(neg_dir)
            .map_err(|e| anyhow!("Failed to read negative directory: {}", e))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("wav") {
                let path_str = path.to_string_lossy();
                
                // Read WAV file duration
                let duration = match WavReader::open(&path) {
                    Ok(r) => {
                        let spec = r.spec();
                        r.len() as f32 / (spec.sample_rate as f32 * spec.channels as f32)
                    }
                    Err(_) => 0.0,
                };
                total_duration_secs += duration;

                match process_wav_file(&path_str, &mut oww_model, !args.no_apm) {
                    Ok(detections) => {
                        if detections > 0 {
                            fa += 1;
                            total_fa_triggers += detections;
                            println!("  [FA] {}: Triggered {} times! (Duration: {:.2}s)", path.file_name().unwrap().to_string_lossy(), detections, duration);
                        } else {
                            tn += 1;
                            println!("  [TN] {}: Clean (Duration: {:.2}s)", path.file_name().unwrap().to_string_lossy(), duration);
                        }
                    }
                    Err(e) => {
                        log::error!("Error processing {}: {:?}", path_str, e);
                    }
                }
            }
        }
        let total_neg = fa + tn;
        if total_neg > 0 {
            let far = (fa as f32 / total_neg as f32) * 100.0;
            let total_hours = total_duration_secs / 3600.0;
            let fa_per_hour = if total_hours > 0.0 {
                total_fa_triggers as f32 / total_hours
            } else {
                0.0
            };
            println!("\nNegative Results:");
            println!("  Total Negative Files: {}", total_neg);
            println!("  True Negatives (TN):  {}", tn);
            println!("  False Acceptances (FA): {}", fa);
            println!("  False Acceptance Rate (FAR): {:.2}%", far);
            println!("  Total Audio Duration: {:.2}s ({:.4} hours)", total_duration_secs, total_hours);
            println!("  False Alarms per Hour: {:.2}\n", fa_per_hour);
        } else {
            println!("No WAV files found in negative directory.\n");
        }
    }

    Ok(())
}
