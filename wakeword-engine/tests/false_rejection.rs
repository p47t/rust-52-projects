use wakeword_engine::{OwwModel, SpeechUnlockType};
use std::path::Path;

fn get_peak_probability(file_path: &str) -> f32 {
    let mut model = OwwModel::new(SpeechUnlockType::OpenWakeWordAlexa, 0.01).unwrap();
    let processed_samples = wakeword_engine::get_processed_samples(file_path, true).unwrap();
    
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
    max_prob
}

#[test]
fn test_frr_on_all_positive_samples() {
    let positives = [
        ("tests/samples/alexa_positive.wav", "Default/US accent Alexa"),
        ("tests/samples/alexa_uk.wav", "UK accent Alexa"),
        ("tests/samples/alexa_au.wav", "Australian accent Alexa"),
        ("tests/samples/alexa_slow.wav", "Slow-paced Alexa"),
        ("tests/samples/alexa_command.wav", "Alexa phrase command"),
    ];

    for &(path, description) in &positives {
        if !Path::new(path).exists() {
            println!("Skipping FRR test for {}: File not found at '{}'", description, path);
            continue;
        }

        let max_prob = get_peak_probability(path);
        println!("Peak probability for {} ({}): {:.4}", description, path, max_prob);

        assert!(
            max_prob >= 0.1,
            "Wake word '{}' should be recognized with at least 0.1 confidence (max was {:.4})",
            description,
            max_prob
        );
    }
}
