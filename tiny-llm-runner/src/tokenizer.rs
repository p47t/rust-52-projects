//! SentencePiece-BPE tokenizer driven by the vocab and scores stored in GGUF.
//!
//! This is the standard llama.cpp llama-tokenizer encoding loop:
//!   1. Map each byte/character of the input string to its token (or to byte
//!      fallback tokens when the character isn't in the vocab).
//!   2. Repeatedly merge the highest-scoring adjacent pair until none of the
//!      remaining adjacencies form a vocab token.

use anyhow::{anyhow, bail, Result};
use llm_gguf_parser::{GgufFile, Value};
use std::collections::HashMap;

pub struct Tokenizer {
    pub tokens: Vec<String>,
    pub scores: Vec<f32>,
    pub token_to_id: HashMap<String, u32>,
    pub bos: u32,
    pub eos: u32,
    /// 256 byte-fallback tokens like `<0xAB>`, when present.
    pub byte_fallback: Option<[u32; 256]>,
}

impl Tokenizer {
    pub fn from_gguf(g: &GgufFile) -> Result<Self> {
        let tokens = match g.metadata.get("tokenizer.ggml.tokens") {
            Some(Value::Array(_, v)) => v
                .iter()
                .map(|x| match x {
                    Value::String(s) => Ok(s.clone()),
                    _ => Err(anyhow!("tokenizer.ggml.tokens contains non-string")),
                })
                .collect::<Result<Vec<_>>>()?,
            _ => bail!("missing tokenizer.ggml.tokens"),
        };
        let scores = match g.metadata.get("tokenizer.ggml.scores") {
            Some(Value::Array(_, v)) => v
                .iter()
                .map(|x| match x {
                    Value::Float32(f) => Ok(*f),
                    _ => Err(anyhow!("tokenizer.ggml.scores contains non-f32")),
                })
                .collect::<Result<Vec<_>>>()?,
            // Models without scores (BPE-style) fall back to a uniform score.
            _ => vec![0.0f32; tokens.len()],
        };

        if scores.len() != tokens.len() {
            bail!(
                "tokens/scores length mismatch: {} vs {}",
                tokens.len(),
                scores.len()
            );
        }

        let mut token_to_id = HashMap::with_capacity(tokens.len());
        for (i, t) in tokens.iter().enumerate() {
            token_to_id.insert(t.clone(), i as u32);
        }

        let bos = get_special(g, "tokenizer.ggml.bos_token_id").unwrap_or(1);
        let eos = get_special(g, "tokenizer.ggml.eos_token_id").unwrap_or(2);

        // Detect byte-fallback tokens: `<0x00>` .. `<0xFF>`.
        let mut byte_fallback = [u32::MAX; 256];
        let mut have_all = true;
        for b in 0..=255u32 {
            let key = format!("<0x{:02X}>", b);
            if let Some(&id) = token_to_id.get(&key) {
                byte_fallback[b as usize] = id;
            } else {
                have_all = false;
            }
        }
        let byte_fallback = if have_all { Some(byte_fallback) } else { None };

        Ok(Self {
            tokens,
            scores,
            token_to_id,
            bos,
            eos,
            byte_fallback,
        })
    }

    pub fn encode(&self, text: &str, add_bos: bool) -> Vec<u32> {
        // SentencePiece prefixes input with a leading space (encoded as ▁).
        let prepared = format!("\u{2581}{}", text.replace(' ', "\u{2581}"));

        // Initialize: one token id per character (or byte-fallback).
        let mut ids: Vec<u32> = Vec::with_capacity(prepared.len());
        for ch in prepared.chars() {
            let s: String = ch.to_string();
            if let Some(&id) = self.token_to_id.get(&s) {
                ids.push(id);
            } else if let Some(bf) = &self.byte_fallback {
                // Encode this character as raw UTF-8 bytes via byte tokens.
                for &byte in s.as_bytes() {
                    let id = bf[byte as usize];
                    if id == u32::MAX {
                        // Should not happen — `byte_fallback` is `Some` only when complete.
                        continue;
                    }
                    ids.push(id);
                }
            }
            // If no byte fallback exists and the char isn't in vocab, drop it.
        }

        // Greedy merge: each round, find the highest-score adjacent pair
        // whose concatenation is a vocab token, and merge it.
        loop {
            let mut best_score = f32::NEG_INFINITY;
            let mut best_idx: Option<usize> = None;
            let mut best_id: u32 = 0;
            for i in 0..ids.len().saturating_sub(1) {
                let merged = format!(
                    "{}{}",
                    &self.tokens[ids[i] as usize],
                    &self.tokens[ids[i + 1] as usize]
                );
                if let Some(&id) = self.token_to_id.get(&merged) {
                    let s = self.scores[id as usize];
                    if s > best_score {
                        best_score = s;
                        best_idx = Some(i);
                        best_id = id;
                    }
                }
            }
            match best_idx {
                Some(i) => {
                    ids[i] = best_id;
                    ids.remove(i + 1);
                }
                None => break,
            }
        }

        if add_bos {
            ids.insert(0, self.bos);
        }
        ids
    }

    /// Render token id `id` as a UTF-8 fragment. Handles the SentencePiece
    /// `▁` → space rewrite and `<0xAB>` byte-fallback decoding.
    pub fn decode_piece(&self, id: u32) -> String {
        let s = match self.tokens.get(id as usize) {
            Some(s) => s,
            None => return String::new(),
        };
        // Byte-fallback: `<0xAB>` → raw byte.
        if s.len() == 6 && s.starts_with("<0x") && s.ends_with('>') {
            if let Ok(b) = u8::from_str_radix(&s[3..5], 16) {
                // Return a 1-char string of that byte; callers concatenate raw
                // and may need to re-stitch into multi-byte UTF-8 themselves.
                let bytes = [b];
                return match std::str::from_utf8(&bytes) {
                    Ok(s) => s.to_string(),
                    Err(_) => {
                        // Lossy: emit replacement char if not valid alone.
                        // Callers wanting byte-perfect output should call
                        // `decode_pieces_bytes`.
                        char::from(b).to_string()
                    }
                };
            }
        }
        s.replace('\u{2581}', " ")
    }

    /// Decode a sequence of ids to a String, correctly stitching byte-fallback
    /// fragments into multi-byte UTF-8 codepoints.
    pub fn decode(&self, ids: &[u32]) -> String {
        let mut bytes: Vec<u8> = Vec::new();
        for &id in ids {
            let s = match self.tokens.get(id as usize) {
                Some(s) => s,
                None => continue,
            };
            if s.len() == 6 && s.starts_with("<0x") && s.ends_with('>') {
                if let Ok(b) = u8::from_str_radix(&s[3..5], 16) {
                    bytes.push(b);
                    continue;
                }
            }
            bytes.extend_from_slice(s.replace('\u{2581}', " ").as_bytes());
        }
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

fn get_special(g: &GgufFile, key: &str) -> Option<u32> {
    match g.metadata.get(key)? {
        Value::Uint8(x) => Some(*x as u32),
        Value::Uint16(x) => Some(*x as u32),
        Value::Uint32(x) => Some(*x),
        Value::Uint64(x) => Some(*x as u32),
        Value::Int32(x) => Some(*x as u32),
        Value::Int64(x) => Some(*x as u32),
        _ => None,
    }
}
