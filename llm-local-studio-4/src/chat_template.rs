#![allow(dead_code)]
//! Chat template module for converting OpenAI-style chat messages into
//! formatted prompt strings suitable for different LLM architectures.
//!
//! Provides [`ChatTemplate`] trait and concrete implementations for ChatML,
//! Llama-3, and a simple generic fallback format. Use [`auto_detect`] to
//! pick the right template based on a model name.

use std::fmt;

use serde::{Deserialize, Serialize};

/// The role of a participant in a chat conversation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Role::System => write!(f, "system"),
            Role::User => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
        }
    }
}

/// A single message in a chat conversation.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

/// Trait for applying a chat template to a sequence of messages.
///
/// Implementations convert a slice of [`ChatMessage`]s into a single prompt
/// string that the model can consume directly.
pub trait ChatTemplate: Send + Sync {
    /// Format the given messages into a prompt string.
    fn apply(&self, messages: &[ChatMessage]) -> String;

    /// A short identifier for this template format.
    fn name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// ChatML template
// ---------------------------------------------------------------------------

/// The ChatML format, widely supported by many fine-tuned models.
///
/// ```text
/// <|im_start|>system
/// You are a helpful assistant.<|im_end|>
/// <|im_start|>user
/// Hello!<|im_end|>
/// <|im_start|>assistant
/// ```
#[derive(Debug, Clone, Default)]
pub struct ChatMLTemplate;

impl ChatTemplate for ChatMLTemplate {
    fn apply(&self, messages: &[ChatMessage]) -> String {
        let mut prompt = String::new();
        for msg in messages {
            prompt.push_str(&format!(
                "<|im_start|>{}\n{}<|im_end|>\n",
                msg.role, msg.content
            ));
        }
        prompt.push_str("<|im_start|>assistant\n");
        prompt
    }

    fn name(&self) -> &str {
        "chatml"
    }
}

// ---------------------------------------------------------------------------
// Llama-3 template
// ---------------------------------------------------------------------------

/// Template for Meta Llama-3 / Llama-3.1 style models.
///
/// ```text
/// <|begin_of_text|><|start_header_id|>system<|end_header_id|>
///
/// You are a helpful assistant.<|eot_id|><|start_header_id|>user<|end_header_id|>
///
/// Hello!<|eot_id|><|start_header_id|>assistant<|end_header_id|>
///
/// ```
#[derive(Debug, Clone, Default)]
pub struct Llama3Template;

impl ChatTemplate for Llama3Template {
    fn apply(&self, messages: &[ChatMessage]) -> String {
        let mut prompt = String::from("<|begin_of_text|>");
        for msg in messages {
            prompt.push_str(&format!(
                "<|start_header_id|>{}<|end_header_id|>\n\n{}<|eot_id|>",
                msg.role, msg.content
            ));
        }
        prompt.push_str("<|start_header_id|>assistant<|end_header_id|>\n\n");
        prompt
    }

    fn name(&self) -> &str {
        "llama3"
    }
}

// ---------------------------------------------------------------------------
// Generic (fallback) template
// ---------------------------------------------------------------------------

/// A simple markdown-style fallback template that works as a lowest-common-
/// denominator format for models without a known chat template.
///
/// ```text
/// You are a helpful assistant.
///
/// ### User:
/// Hello!
///
/// ### Assistant:
/// ```
#[derive(Debug, Clone, Default)]
pub struct GenericTemplate;

impl ChatTemplate for GenericTemplate {
    fn apply(&self, messages: &[ChatMessage]) -> String {
        let mut prompt = String::new();
        for msg in messages {
            match msg.role {
                Role::System => {
                    prompt.push_str(&msg.content);
                    prompt.push_str("\n\n");
                }
                Role::User => {
                    prompt.push_str("### User:\n");
                    prompt.push_str(&msg.content);
                    prompt.push_str("\n\n");
                }
                Role::Assistant => {
                    prompt.push_str("### Assistant:\n");
                    prompt.push_str(&msg.content);
                    prompt.push_str("\n\n");
                }
            }
        }
        prompt.push_str("### Assistant:\n");
        prompt
    }

    fn name(&self) -> &str {
        "generic"
    }
}

// ---------------------------------------------------------------------------
// Auto-detection
// ---------------------------------------------------------------------------

/// Pick an appropriate [`ChatTemplate`] implementation based on the model name.
///
/// Heuristic (case-insensitive):
/// - Names containing `"llama-3"` or `"llama3"` → [`Llama3Template`]
/// - Names containing `"chatml"` or `"im_start"` → [`ChatMLTemplate`]
/// - Everything else → [`ChatMLTemplate`] (safe default for most fine-tunes)
pub fn auto_detect(model_name: &str) -> Box<dyn ChatTemplate> {
    let lower = model_name.to_lowercase();

    if lower.contains("llama-3") || lower.contains("llama3") {
        Box::new(Llama3Template)
    } else {
        Box::new(ChatMLTemplate)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_messages() -> Vec<ChatMessage> {
        vec![
            ChatMessage {
                role: Role::System,
                content: "You are a helpful assistant.".into(),
            },
            ChatMessage {
                role: Role::User,
                content: "Hello!".into(),
            },
        ]
    }

    #[test]
    fn chatml_template_formats_correctly() {
        let template = ChatMLTemplate;
        let output = template.apply(&sample_messages());

        let expected = "\
<|im_start|>system
You are a helpful assistant.<|im_end|>
<|im_start|>user
Hello!<|im_end|>
<|im_start|>assistant
";
        assert_eq!(output, expected);
        assert_eq!(template.name(), "chatml");
    }

    #[test]
    fn llama3_template_formats_correctly() {
        let template = Llama3Template;
        let output = template.apply(&sample_messages());

        let expected = "\
<|begin_of_text|>\
<|start_header_id|>system<|end_header_id|>\n\n\
You are a helpful assistant.\
<|eot_id|>\
<|start_header_id|>user<|end_header_id|>\n\n\
Hello!\
<|eot_id|>\
<|start_header_id|>assistant<|end_header_id|>\n\n";
        assert_eq!(output, expected);
        assert_eq!(template.name(), "llama3");
    }

    #[test]
    fn generic_template_formats_correctly() {
        let template = GenericTemplate;
        let output = template.apply(&sample_messages());

        let expected = "\
You are a helpful assistant.\n\n\
### User:\n\
Hello!\n\n\
### Assistant:\n";
        assert_eq!(output, expected);
        assert_eq!(template.name(), "generic");
    }

    #[test]
    fn auto_detect_picks_llama3_for_llama_model() {
        let template = auto_detect("Meta-Llama-3-8B");
        assert_eq!(template.name(), "llama3");
    }

    #[test]
    fn auto_detect_picks_llama3_case_insensitive() {
        let template = auto_detect("llama3-instruct-v2");
        assert_eq!(template.name(), "llama3");
    }

    #[test]
    fn auto_detect_picks_chatml_for_chatml_model() {
        let template = auto_detect("my-model-chatml");
        assert_eq!(template.name(), "chatml");
    }

    #[test]
    fn auto_detect_defaults_to_chatml_for_unknown() {
        let template = auto_detect("some-random-model");
        assert_eq!(template.name(), "chatml");
    }

    #[test]
    fn role_deserialize_from_lowercase() {
        let system: Role = serde_json::from_str(r#""system""#).unwrap();
        assert_eq!(system, Role::System);

        let user: Role = serde_json::from_str(r#""user""#).unwrap();
        assert_eq!(user, Role::User);

        let assistant: Role = serde_json::from_str(r#""assistant""#).unwrap();
        assert_eq!(assistant, Role::Assistant);
    }

    #[test]
    fn role_serialize_to_lowercase() {
        assert_eq!(serde_json::to_string(&Role::System).unwrap(), r#""system""#);
        assert_eq!(serde_json::to_string(&Role::User).unwrap(), r#""user""#);
        assert_eq!(
            serde_json::to_string(&Role::Assistant).unwrap(),
            r#""assistant""#
        );
    }

    #[test]
    fn role_display() {
        assert_eq!(Role::System.to_string(), "system");
        assert_eq!(Role::User.to_string(), "user");
        assert_eq!(Role::Assistant.to_string(), "assistant");
    }
}
