use crate::types::{CliMessage, ContentBlock, MessageContent, MessagesRequest};

#[test]
fn deserialize_request_with_string_content() {
    let json = r#"{
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 100,
        "messages": [{"role": "user", "content": "Hello!"}]
    }"#;

    let req: MessagesRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.model, "claude-sonnet-4-20250514");
    assert!(req.stream); // default true
    assert_eq!(req.messages.len(), 1);
    assert_eq!(req.messages[0].role, "user");
    match &req.messages[0].content {
        MessageContent::Text(s) => assert_eq!(s, "Hello!"),
        _ => panic!("expected Text variant"),
    }
}

#[test]
fn deserialize_request_with_block_content() {
    let json = r#"{
        "model": "claude-opus-4-20250514",
        "max_tokens": 200,
        "messages": [{
            "role": "user",
            "content": [{"type": "text", "text": "Hi there"}]
        }],
        "stream": false
    }"#;

    let req: MessagesRequest = serde_json::from_str(json).unwrap();
    assert!(!req.stream);
    match &req.messages[0].content {
        MessageContent::Blocks(blocks) => {
            assert_eq!(blocks.len(), 1);
            match &blocks[0] {
                ContentBlock::Text { text } => assert_eq!(text, "Hi there"),
                _ => panic!("expected Text block"),
            }
        }
        _ => panic!("expected Blocks variant"),
    }
}

#[test]
fn deserialize_request_with_system_string() {
    let json = r#"{
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 100,
        "system": "You are helpful.",
        "messages": [{"role": "user", "content": "Hi"}]
    }"#;

    let req: MessagesRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.system.unwrap().as_str().unwrap(), "You are helpful.");
}

#[test]
fn deserialize_request_with_system_blocks() {
    let json = r#"{
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 100,
        "system": [{"type": "text", "text": "System prompt here"}],
        "messages": [{"role": "user", "content": "Hi"}]
    }"#;

    let req: MessagesRequest = serde_json::from_str(json).unwrap();
    let system = req.system.unwrap();
    assert!(system.is_array());
    assert_eq!(system[0]["text"].as_str().unwrap(), "System prompt here");
}

#[test]
fn deserialize_cli_system_message() {
    let json = r#"{"type": "system", "model": "claude-sonnet-4-6"}"#;
    let msg: CliMessage = serde_json::from_str(json).unwrap();
    match msg {
        CliMessage::System { model } => assert_eq!(model.unwrap(), "claude-sonnet-4-6"),
        _ => panic!("expected System variant"),
    }
}

#[test]
fn deserialize_cli_assistant_message() {
    let json = r#"{
        "type": "assistant",
        "message": {
            "id": "msg_123",
            "model": "claude-sonnet-4-6",
            "content": [{"type": "text", "text": "Hello!"}],
            "usage": {"input_tokens": 10, "output_tokens": 5}
        }
    }"#;

    let msg: CliMessage = serde_json::from_str(json).unwrap();
    match msg {
        CliMessage::Assistant { message } => {
            assert_eq!(message.id.unwrap(), "msg_123");
            assert_eq!(message.content.len(), 1);
            match &message.content[0] {
                ContentBlock::Text { text } => assert_eq!(text, "Hello!"),
                _ => panic!("expected Text block"),
            }
            let usage = message.usage.unwrap();
            assert_eq!(usage.input_tokens, 10);
            assert_eq!(usage.output_tokens, 5);
        }
        _ => panic!("expected Assistant variant"),
    }
}

#[test]
fn deserialize_cli_result_message() {
    let json = r#"{
        "type": "result",
        "stop_reason": "end_turn",
        "usage": {
            "input_tokens": 100,
            "output_tokens": 50,
            "cache_read_input_tokens": 0,
            "cache_creation_input_tokens": 0
        }
    }"#;

    let msg: CliMessage = serde_json::from_str(json).unwrap();
    match msg {
        CliMessage::Result { stop_reason, usage } => {
            assert_eq!(stop_reason.unwrap(), "end_turn");
            let u = usage.unwrap();
            assert_eq!(u.input_tokens, 100);
            assert_eq!(u.output_tokens, 50);
        }
        _ => panic!("expected Result variant"),
    }
}

#[test]
fn deserialize_cli_rate_limit_event() {
    let json = r#"{"type": "rate_limit_event"}"#;
    let msg: CliMessage = serde_json::from_str(json).unwrap();
    assert!(matches!(msg, CliMessage::RateLimitEvent {}));
}

#[test]
fn stream_defaults_to_true() {
    let json = r#"{
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 100,
        "messages": [{"role": "user", "content": "Hi"}]
    }"#;

    let req: MessagesRequest = serde_json::from_str(json).unwrap();
    assert!(req.stream);
}

#[test]
fn deserialize_real_cli_result_with_extra_fields() {
    // Actual format from `claude -p --output-format stream-json --verbose`
    let json = r#"{
        "type": "result",
        "subtype": "success",
        "is_error": false,
        "duration_ms": 7914,
        "duration_api_ms": 7894,
        "num_turns": 1,
        "result": "Hi!",
        "stop_reason": "end_turn",
        "session_id": "9adef40e-0a65-4d52-a2e5-610a67a9396b",
        "total_cost_usd": 0.03112375,
        "usage": {
            "input_tokens": 10,
            "cache_creation_input_tokens": 24595,
            "cache_read_input_tokens": 0,
            "output_tokens": 74,
            "server_tool_use": {"web_search_requests": 0, "web_fetch_requests": 0},
            "service_tier": "standard",
            "cache_creation": {"ephemeral_1h_input_tokens": 24595, "ephemeral_5m_input_tokens": 0},
            "inference_geo": "",
            "iterations": [],
            "speed": "standard"
        },
        "modelUsage": {},
        "permission_denials": [],
        "fast_mode_state": "off",
        "uuid": "18c57cb6-fc9a-4b4c-a1bd-c1651b5fd2b8"
    }"#;

    let msg: CliMessage = serde_json::from_str(json).unwrap();
    match msg {
        CliMessage::Result { stop_reason, usage } => {
            assert_eq!(stop_reason.unwrap(), "end_turn");
            let u = usage.unwrap();
            assert_eq!(u.input_tokens, 10);
            assert_eq!(u.output_tokens, 74);
            assert_eq!(u.cache_creation_input_tokens, 24595);
        }
        _ => panic!("expected Result variant"),
    }
}

#[test]
fn deserialize_real_cli_system_init() {
    let json = r#"{
        "type": "system",
        "subtype": "init",
        "cwd": "/Users/test",
        "session_id": "abc",
        "tools": ["Bash", "Read"],
        "model": "claude-haiku-4-5-20251001",
        "permissionMode": "default",
        "uuid": "123"
    }"#;

    let msg: CliMessage = serde_json::from_str(json).unwrap();
    match msg {
        CliMessage::System { model } => assert_eq!(model.unwrap(), "claude-haiku-4-5-20251001"),
        _ => panic!("expected System variant"),
    }
}

#[test]
fn deserialize_real_cli_assistant_with_thinking() {
    let json = r#"{
        "type": "assistant",
        "message": {
            "model": "claude-haiku-4-5-20251001",
            "id": "msg_test",
            "type": "message",
            "role": "assistant",
            "content": [{"type": "thinking", "thinking": "Let me think...", "signature": "EugCCk..."}],
            "stop_reason": null,
            "stop_sequence": null,
            "usage": {"input_tokens": 10, "cache_creation_input_tokens": 24595, "cache_read_input_tokens": 0, "output_tokens": 8, "service_tier": "standard"},
            "context_management": null
        },
        "parent_tool_use_id": null,
        "session_id": "abc",
        "uuid": "def"
    }"#;

    let msg: CliMessage = serde_json::from_str(json).unwrap();
    match msg {
        CliMessage::Assistant { message } => {
            assert_eq!(message.content.len(), 1);
            match &message.content[0] {
                ContentBlock::Thinking { thinking } => assert_eq!(thinking, "Let me think..."),
                _ => panic!("expected Thinking block"),
            }
        }
        _ => panic!("expected Assistant variant"),
    }
}

#[test]
fn unknown_cli_fields_are_skipped() {
    let json = r#"{
        "type": "system",
        "model": "claude-sonnet-4-6",
        "session_id": "sess_abc",
        "some_extra_field": 42
    }"#;

    // Should not fail — unknown fields are ignored
    let msg: Result<CliMessage, _> = serde_json::from_str(json);
    assert!(msg.is_ok());
}
