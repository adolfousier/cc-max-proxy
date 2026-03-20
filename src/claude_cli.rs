use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::error::ProxyError;
use crate::types::{CliMessage, MessageContent, MessagesRequest};

/// Resolve the claude CLI binary path.
pub fn resolve_claude_path() -> Result<String, ProxyError> {
    if let Ok(path) = std::env::var("CLAUDE_PATH") {
        if std::path::Path::new(&path).exists() {
            return Ok(path);
        }
        return Err(ProxyError::CliNotFound(path));
    }

    // Try common locations
    for candidate in &["/opt/homebrew/bin/claude", "/usr/local/bin/claude"] {
        if std::path::Path::new(candidate).exists() {
            return Ok(candidate.to_string());
        }
    }

    // Try PATH via `which`
    if let Ok(output) = std::process::Command::new("which").arg("claude").output()
        && output.status.success()
    {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Ok(path);
        }
    }

    Err(ProxyError::CliNotFound("claude".to_string()))
}

/// Map an Anthropic model string to a claude CLI model flag.
pub(crate) fn map_model(model: &str) -> &str {
    if model.contains("opus") {
        "opus"
    } else if model.contains("haiku") {
        "haiku"
    } else {
        "sonnet"
    }
}

/// Build the prompt string from the Anthropic request.
pub(crate) fn build_prompt(request: &MessagesRequest) -> String {
    let mut parts = Vec::new();

    // System prompt
    if let Some(ref system) = request.system {
        match system {
            serde_json::Value::String(s) => parts.push(s.clone()),
            serde_json::Value::Array(blocks) => {
                let text: String = blocks
                    .iter()
                    .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("\n");
                if !text.is_empty() {
                    parts.push(text);
                }
            }
            _ => {}
        }
    }

    // Conversation messages
    for msg in &request.messages {
        let role = match msg.role.as_str() {
            "assistant" => "Assistant",
            _ => "Human",
        };
        let content = match &msg.content {
            MessageContent::Text(s) => s.clone(),
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|b| match b {
                    crate::types::ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(""),
        };
        parts.push(format!("{}: {}", role, content));
    }

    parts.join("\n\n")
}

/// Spawn the claude CLI and stream NDJSON messages back via a channel.
pub async fn spawn_stream(
    claude_path: &str,
    request: &MessagesRequest,
) -> Result<mpsc::Receiver<Result<CliMessage, ProxyError>>, ProxyError> {
    let prompt = build_prompt(request);
    let model = map_model(&request.model);

    tracing::info!(
        "Spawning claude CLI: model={}, prompt_len={}",
        model,
        prompt.len()
    );

    let mut child = Command::new(claude_path)
        .arg("-p")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .arg("--no-session-persistence")
        .arg("--model")
        .arg(model)
        .arg("--")
        .arg(&prompt)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ProxyError::CliError("failed to capture stdout".to_string()))?;

    let (tx, rx) = mpsc::channel::<Result<CliMessage, ProxyError>>(32);

    tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }

            match serde_json::from_str::<CliMessage>(&line) {
                Ok(msg) => {
                    if tx.send(Ok(msg)).await.is_err() {
                        break; // receiver dropped
                    }
                }
                Err(e) => {
                    tracing::debug!(
                        "Skipping unparseable CLI line: {} — {}",
                        e,
                        &line[..line.len().min(200)]
                    );
                }
            }
        }

        // Wait for process to exit
        match child.wait().await {
            Ok(status) if !status.success() => {
                tracing::warn!("claude CLI exited with status: {}", status);
            }
            Err(e) => {
                tracing::error!("Failed to wait on claude CLI: {}", e);
            }
            _ => {}
        }
    });

    Ok(rx)
}
