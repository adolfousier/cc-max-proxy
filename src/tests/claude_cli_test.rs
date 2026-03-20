use crate::claude_cli::{build_prompt, map_model};
use crate::types::{Message, MessageContent, MessagesRequest};

#[test]
fn map_model_opus() {
    assert_eq!(map_model("claude-opus-4-20250514"), "opus");
    assert_eq!(map_model("claude-opus-4-6"), "opus");
}

#[test]
fn map_model_haiku() {
    assert_eq!(map_model("claude-haiku-4-5-20251001"), "haiku");
    assert_eq!(map_model("claude-3-haiku"), "haiku");
}

#[test]
fn map_model_sonnet() {
    assert_eq!(map_model("claude-sonnet-4-20250514"), "sonnet");
    assert_eq!(map_model("claude-sonnet-4-6"), "sonnet");
}

#[test]
fn map_model_defaults_to_sonnet() {
    assert_eq!(map_model("claude-4"), "sonnet");
    assert_eq!(map_model("some-unknown-model"), "sonnet");
}

#[test]
fn build_prompt_simple_message() {
    let req = MessagesRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: MessageContent::Text("Hello!".to_string()),
        }],
        system: None,
        stream: true,
    };

    let prompt = build_prompt(&req);
    assert_eq!(prompt, "Human: Hello!");
}

#[test]
fn build_prompt_with_system_string() {
    let req = MessagesRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: MessageContent::Text("Hi".to_string()),
        }],
        system: Some(serde_json::Value::String("Be helpful.".to_string())),
        stream: true,
    };

    let prompt = build_prompt(&req);
    assert!(prompt.starts_with("Be helpful."));
    assert!(prompt.contains("Human: Hi"));
}

#[test]
fn build_prompt_with_system_blocks() {
    let req = MessagesRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: MessageContent::Text("Hi".to_string()),
        }],
        system: Some(serde_json::json!([
            {"type": "text", "text": "Block one"},
            {"type": "text", "text": "Block two"}
        ])),
        stream: true,
    };

    let prompt = build_prompt(&req);
    assert!(prompt.starts_with("Block one\nBlock two"));
}

#[test]
fn build_prompt_multi_turn() {
    let req = MessagesRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        messages: vec![
            Message {
                role: "user".to_string(),
                content: MessageContent::Text("Hello".to_string()),
            },
            Message {
                role: "assistant".to_string(),
                content: MessageContent::Text("Hi there!".to_string()),
            },
            Message {
                role: "user".to_string(),
                content: MessageContent::Text("How are you?".to_string()),
            },
        ],
        system: None,
        stream: true,
    };

    let prompt = build_prompt(&req);
    assert!(prompt.contains("Human: Hello"));
    assert!(prompt.contains("Assistant: Hi there!"));
    assert!(prompt.contains("Human: How are you?"));
}

#[test]
fn cli_path_env_override() {
    // Set CLAUDE_PATH to a non-existent path — should return error
    // SAFETY: single-threaded test, no other threads reading this env var
    unsafe {
        std::env::set_var("CLAUDE_PATH", "/tmp/nonexistent-claude-binary");
    }
    let result = crate::claude_cli::resolve_claude_path();
    unsafe {
        std::env::remove_var("CLAUDE_PATH");
    }
    assert!(result.is_err());
}
