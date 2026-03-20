use serde::{Deserialize, Serialize};

// ── Incoming: Anthropic Messages API request ──

#[derive(Debug, Deserialize)]
pub struct MessagesRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub system: Option<serde_json::Value>,
    #[serde(default = "default_stream")]
    pub stream: bool,
}

fn default_stream() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: MessageContent,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Image {
        source: serde_json::Value,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

// ── CLI NDJSON output types ──

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CliMessage {
    System {
        model: Option<String>,
    },
    Assistant {
        message: CliAssistantMessage,
    },
    RateLimitEvent {},
    Result {
        stop_reason: Option<String>,
        usage: Option<CliUsage>,
    },
}

#[derive(Debug, Deserialize)]
pub struct CliAssistantMessage {
    pub id: Option<String>,
    pub model: Option<String>,
    pub content: Vec<ContentBlock>,
    pub usage: Option<CliUsage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CliUsage {
    #[serde(default)]
    pub input_tokens: u32,
    #[serde(default)]
    pub output_tokens: u32,
    #[serde(default)]
    pub cache_read_input_tokens: u32,
    #[serde(default)]
    pub cache_creation_input_tokens: u32,
}

// ── Outgoing: Anthropic SSE events ──

#[derive(Debug, Serialize)]
pub struct SseMessageStart {
    pub r#type: &'static str,
    pub message: SseMessageMeta,
}

#[derive(Debug, Serialize)]
pub struct SseMessageMeta {
    pub id: String,
    pub r#type: &'static str,
    pub role: &'static str,
    pub model: String,
    pub content: Vec<()>,
    pub stop_reason: Option<String>,
    pub usage: SseUsage,
}

#[derive(Debug, Clone, Serialize)]
pub struct SseUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct SseContentBlockStart {
    pub r#type: &'static str,
    pub index: usize,
    pub content_block: ContentBlock,
}

#[derive(Debug, Serialize)]
pub struct SseContentBlockDelta {
    pub r#type: &'static str,
    pub index: usize,
    pub delta: SseDelta,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseDelta {
    TextDelta { text: String },
}

#[derive(Debug, Serialize)]
pub struct SseContentBlockStop {
    pub r#type: &'static str,
    pub index: usize,
}

#[derive(Debug, Serialize)]
pub struct SseMessageDelta {
    pub r#type: &'static str,
    pub delta: SseMessageDeltaInner,
    pub usage: SseUsage,
}

#[derive(Debug, Serialize)]
pub struct SseMessageDeltaInner {
    pub stop_reason: String,
    pub stop_sequence: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SseMessageStop {
    pub r#type: &'static str,
}

// ── Non-streaming response ──

#[derive(Debug, Serialize)]
pub struct MessagesResponse {
    pub id: String,
    pub r#type: &'static str,
    pub role: &'static str,
    pub model: String,
    pub content: Vec<ContentBlock>,
    pub stop_reason: String,
    pub usage: SseUsage,
}
