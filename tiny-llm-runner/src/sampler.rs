//! Token sampling: greedy or temperature/top-k with a small xorshift PRNG.

use crate::ops::softmax;

pub struct Sampler {
    pub temperature: f32,
    pub top_k: usize,
    rng_state: u64,
}

impl Sampler {
    pub fn new(temperature: f32, top_k: usize, seed: u64) -> Self {
        // Avoid the all-zero state.
        let s = if seed == 0 { 0x9E3779B97F4A7C15 } else { seed };
        Self {
            temperature,
            top_k,
            rng_state: s,
        }
    }

    pub fn sample(&mut self, logits: &mut [f32]) -> u32 {
        if self.temperature <= 0.0 {
            return argmax(logits) as u32;
        }
        // Apply temperature.
        let inv_t = 1.0 / self.temperature;
        for v in logits.iter_mut() {
            *v *= inv_t;
        }

        // Top-k filtering: zero out everything except the k largest.
        if self.top_k > 0 && self.top_k < logits.len() {
            let mut indexed: Vec<(usize, f32)> = logits.iter().copied().enumerate().collect();
            // Partial sort: bring the top-k to the front.
            indexed.select_nth_unstable_by(self.top_k, |a, b| {
                b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
            });
            let cutoff = indexed[..self.top_k]
                .iter()
                .map(|(_, v)| *v)
                .fold(f32::INFINITY, f32::min);
            for v in logits.iter_mut() {
                if *v < cutoff {
                    *v = f32::NEG_INFINITY;
                }
            }
        }

        softmax(logits);
        let r = self.next_f32();
        let mut acc = 0.0f32;
        for (i, &p) in logits.iter().enumerate() {
            acc += p;
            if acc >= r {
                return i as u32;
            }
        }
        (logits.len() - 1) as u32
    }

    fn next_u64(&mut self) -> u64 {
        // xorshift64.
        let mut s = self.rng_state;
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        self.rng_state = s;
        s
    }

    fn next_f32(&mut self) -> f32 {
        // 24 bits → [0, 1).
        let bits = (self.next_u64() >> 40) as u32;
        bits as f32 / (1u32 << 24) as f32
    }
}

fn argmax(x: &[f32]) -> usize {
    let mut best = 0usize;
    let mut best_v = f32::NEG_INFINITY;
    for (i, &v) in x.iter().enumerate() {
        if v > best_v {
            best_v = v;
            best = i;
        }
    }
    best
}
