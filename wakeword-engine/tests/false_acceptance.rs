use hound::{WavSpec, WavWriter};
use std::fs;
use std::path::Path;
use wakeword_engine::{OwwModel, SpeechUnlockType, process_wav_file};

fn ensure_test_samples() {
    let dir = Path::new("tests/samples");
    if !dir.exists() {
        fs::create_dir_all(dir).unwrap();
    }

    // 1. Generate silence.wav (16 kHz, 3 seconds)
    let silence_path = dir.join("silence.wav");
    if !silence_path.exists() {
        let spec = WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = WavWriter::create(silence_path, spec).unwrap();
        for _ in 0..(16000 * 3) {
            writer.write_sample(0i16).unwrap();
        }
        writer.finalize().unwrap();
    }

    // 2. Generate noise.wav (16 kHz, 3 seconds white noise)
    let noise_path = dir.join("noise.wav");
    if !noise_path.exists() {
        let spec = WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = WavWriter::create(noise_path, spec).unwrap();
        use rand::Rng;
        let mut rng = rand::thread_rng();
        for _ in 0..(16000 * 3) {
            let sample = rng.gen_range(-0.1f32..0.1f32);
            let s = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
            writer.write_sample(s).unwrap();
        }
        writer.finalize().unwrap();
    }

    // 3. Generate sine_48khz.wav (48 kHz, 3 seconds of 440 Hz)
    // This explicitly tests our resampler (48 kHz -> 16 kHz)
    let sine_path = dir.join("sine_48khz.wav");
    if !sine_path.exists() {
        let spec = WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = WavWriter::create(sine_path, spec).unwrap();
        let freq = 440.0f32;
        for i in 0..(48000 * 3) {
            let sample = (i as f32 * freq * 2.0 * std::f32::consts::PI / 48000.0).sin() * 0.5;
            let s = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
            writer.write_sample(s).unwrap();
        }
        writer.finalize().unwrap();
    }
}

#[test]
fn test_far_on_silence_file() {
    ensure_test_samples();
    let mut model = OwwModel::new(SpeechUnlockType::OpenWakeWordAlexa, 0.5).unwrap();
    let detections = process_wav_file("tests/samples/silence.wav", &mut model, true).unwrap();
    assert_eq!(detections, 0, "Silence WAV file should never trigger the wake word (FAR = 0%)");
}

#[test]
fn test_far_on_white_noise_file() {
    ensure_test_samples();
    let mut model = OwwModel::new(SpeechUnlockType::OpenWakeWordAlexa, 0.5).unwrap();
    let detections = process_wav_file("tests/samples/noise.wav", &mut model, true).unwrap();
    assert_eq!(detections, 0, "White noise WAV file should never trigger the wake word (FAR = 0%)");
}

#[test]
fn test_far_on_sine_wave_48khz_file() {
    ensure_test_samples();
    let mut model = OwwModel::new(SpeechUnlockType::OpenWakeWordAlexa, 0.5).unwrap();
    let detections = process_wav_file("tests/samples/sine_48khz.wav", &mut model, true).unwrap();
    assert_eq!(detections, 0, "Sine wave 48 kHz WAV file (resampled) should never trigger the wake word (FAR = 0%)");
}

#[test]
fn test_far_on_all_negative_speech_samples() {
    let negatives = [
        ("tests/samples/english_negative.wav", "English speech sentences"),
        ("tests/samples/near_miss_alex.wav", "Near miss 'Alex'"),
        ("tests/samples/near_miss_alexis.wav", "Near miss 'Alexis'"),
        ("tests/samples/near_miss_alexander.wav", "Near miss 'Alexander'"),
        ("tests/samples/negative_computer.wav", "Command starting with 'Computer'"),
        ("tests/samples/negative_google.wav", "Command starting with 'Hey Google'"),
        ("tests/samples/negative_siri.wav", "Command starting with 'Hey Siri'"),
    ];

    for &(path, description) in &negatives {
        if !std::path::Path::new(path).exists() {
            println!("Skipping negative speech test for {}: File not found at '{}'", description, path);
            continue;
        }

        // Measure the peak probability in the negative speech file
        let mut model = OwwModel::new(SpeechUnlockType::OpenWakeWordAlexa, 0.01).unwrap();
        let processed_samples = wakeword_engine::get_processed_samples(path, true).unwrap();
        
        let chunk_size = wakeword_engine::OWW_MODEL_CHUNK_SIZE;
        let total_samples = processed_samples.len();
        let mut offset = 0;
        let mut max_prob = 0.0f32;

        while offset + chunk_size <= total_samples {
            let chunk = &processed_samples[offset..offset + chunk_size];
            let triggered = model.detection(chunk.to_vec());
            if triggered.probability > max_prob {
                max_prob = triggered.probability;
            }
            offset += chunk_size;
        }

        println!("Maximum 'Alexa' wake word probability in {} ({}): {:.4}", description, path, max_prob);

        // Verify it doesn't trigger with threshold 0.5
        let mut model_std = OwwModel::new(SpeechUnlockType::OpenWakeWordAlexa, 0.5).unwrap();
        let detections = process_wav_file(path, &mut model_std, true).unwrap();
        assert_eq!(
            detections, 0,
            "Negative sample '{}' should not trigger the wake word (detections: {}, max_prob: {:.4})",
            description, detections, max_prob
        );
    }
}
