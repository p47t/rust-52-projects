use wakeword_engine::{
    WakewordPipeline, DeviceProfile, SpeechUnlockType, DetectionResult
};
use std::path::Path;
use std::time::Duration;

fn evaluate_pipeline_on_file(
    profile: DeviceProfile,
    file_path: &str,
    threshold: f32,
) -> Vec<DetectionResult> {
    let mut pipeline = WakewordPipeline::new(
        profile,
        threshold,
        SpeechUnlockType::OpenWakeWordAlexa,
        16000,
        1,
    ).unwrap();

    // Sleep for 2.1 seconds to bypass openWakeWord's 2-second rate-limiting throttle 
    // (NO_DETECTION_MS = 2000) from the model initialization time.
    std::thread::sleep(Duration::from_millis(2100));

    let mut reader = hound::WavReader::open(file_path).unwrap();
    let spec = reader.spec();
    let max_val = (1 << (spec.bits_per_sample - 1)) as f32;
    let mut samples: Vec<f32> = reader.samples::<i32>()
        .filter_map(Result::ok)
        .map(|s| s as f32 / max_val)
        .collect();

    // Append 4.0 seconds of silence (64,000 samples)
    samples.extend(std::iter::repeat(0.0f32).take(64000));

    let mut all_results = Vec::new();
    for chunk in samples.chunks(320) {
        let results = pipeline.process(chunk).unwrap();
        all_results.extend(results);
    }
    
    all_results
}

#[test]
fn test_pipeline_low_power_profile() {
    let pos_path = "tests/samples/alexa_positive.wav";
    let neg_path = "tests/samples/english_negative.wav";

    if !Path::new(pos_path).exists() || !Path::new(neg_path).exists() {
        println!("Skipping pipeline test: Test samples not found.");
        return;
    }

    // 1. Test positive sample triggers wake word with LowPower profile
    let results_pos = evaluate_pipeline_on_file(DeviceProfile::LowPower, pos_path, 0.1);
    let triggered_pos = results_pos.iter().any(|r| r.detected);
    let max_prob_pos = results_pos.iter().map(|r| r.probability).fold(0.0f32, f32::max);
    
    println!("[LowPower] Positive sample max probability: {:.4}", max_prob_pos);
    assert!(triggered_pos, "LowPower profile should trigger on positive sample (max prob: {:.4})", max_prob_pos);

    // 2. Test negative sample does not trigger wake word with LowPower profile
    let results_neg = evaluate_pipeline_on_file(DeviceProfile::LowPower, neg_path, 0.5);
    let triggered_neg = results_neg.iter().any(|r| r.detected);
    let max_prob_neg = results_neg.iter().map(|r| r.probability).fold(0.0f32, f32::max);

    println!("[LowPower] Negative sample max probability: {:.4}", max_prob_neg);
    assert!(!triggered_neg, "LowPower profile should not trigger on negative sample (max prob: {:.4})", max_prob_neg);
}

#[test]
fn test_pipeline_standard_profile() {
    let pos_path = "tests/samples/alexa_positive.wav";
    let neg_path = "tests/samples/english_negative.wav";

    if !Path::new(pos_path).exists() || !Path::new(neg_path).exists() {
        return;
    }

    // 1. Test positive sample triggers wake word with Standard profile (Lightweight APM)
    let results_pos = evaluate_pipeline_on_file(DeviceProfile::Standard, pos_path, 0.1);
    let triggered_pos = results_pos.iter().any(|r| r.detected);
    let max_prob_pos = results_pos.iter().map(|r| r.probability).fold(0.0f32, f32::max);
    
    println!("[Standard] Positive sample max probability: {:.4}", max_prob_pos);
    assert!(triggered_pos, "Standard profile should trigger on positive sample (max prob: {:.4})", max_prob_pos);

    // 2. Test negative sample does not trigger wake word with Standard profile
    let results_neg = evaluate_pipeline_on_file(DeviceProfile::Standard, neg_path, 0.5);
    let triggered_neg = results_neg.iter().any(|r| r.detected);
    let max_prob_neg = results_neg.iter().map(|r| r.probability).fold(0.0f32, f32::max);

    println!("[Standard] Negative sample max probability: {:.4}", max_prob_neg);
    assert!(!triggered_neg, "Standard profile should not trigger on negative sample (max prob: {:.4})", max_prob_neg);
}

#[test]
fn test_pipeline_high_performance_profile() {
    let pos_path = "tests/samples/alexa_positive.wav";
    let neg_path = "tests/samples/english_negative.wav";

    if !Path::new(pos_path).exists() || !Path::new(neg_path).exists() {
        return;
    }

    // 1. Test positive sample triggers wake word with HighPerformance profile (Full WebRTC APM + AGC2)
    let results_pos = evaluate_pipeline_on_file(DeviceProfile::HighPerformance, pos_path, 0.1);
    let triggered_pos = results_pos.iter().any(|r| r.detected);
    let max_prob_pos = results_pos.iter().map(|r| r.probability).fold(0.0f32, f32::max);
    
    println!("[HighPerformance] Positive sample max probability: {:.4}", max_prob_pos);
    assert!(triggered_pos, "HighPerformance profile should trigger on positive sample (max prob: {:.4})", max_prob_pos);

    // 2. Test negative sample does not trigger wake word with HighPerformance profile
    let results_neg = evaluate_pipeline_on_file(DeviceProfile::HighPerformance, neg_path, 0.5);
    let triggered_neg = results_neg.iter().any(|r| r.detected);
    let max_prob_neg = results_neg.iter().map(|r| r.probability).fold(0.0f32, f32::max);

    println!("[HighPerformance] Negative sample max probability: {:.4}", max_prob_neg);
    assert!(!triggered_neg, "HighPerformance profile should not trigger on negative sample (max prob: {:.4})", max_prob_neg);
}
