use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::ai::message::ChatMessage;
use crate::ai::provider::{AiProvider, StreamDelta};
use crate::ai::streaming::parse_sse_events;
use crate::error::AppError;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MAX_TOKENS: u32 = 4096;

pub struct ClaudeProvider {
    client: Client,
    api_key: String,
    model: String,
}

impl ClaudeProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
        }
    }

    /// Separate system messages from the conversation and build the request.
    fn build_request(
        &self,
        messages: &[ChatMessage],
        stream: bool,
    ) -> (Option<String>, Vec<ClaudeMessage>) {
        let mut system_text: Option<String> = None;
        let mut claude_messages = Vec::new();

        for msg in messages {
            if msg.role == "system" {
                // Anthropic API uses a top-level `system` parameter rather than a system role message.
                // Concatenate all system messages.
                match &mut system_text {
                    Some(existing) => {
                        existing.push_str("\n\n");
                        existing.push_str(&msg.content);
                    }
                    None => {
                        system_text = Some(msg.content.clone());
                    }
                }
            } else {
                claude_messages.push(ClaudeMessage {
                    role: msg.role.clone(),
                    content: msg.content.clone(),
                });
            }
        }

        let _ = stream; // used by caller, not here
        (system_text, claude_messages)
    }
}

// --- Request / Response types for the Anthropic Messages API ---

#[derive(Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<ClaudeMessage>,
    stream: bool,
}

#[derive(Serialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct ClaudeResponse {
    content: Vec<ClaudeContentBlock>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct ClaudeContentBlock {
    text: Option<String>,
}

/// SSE event envelope from the Anthropic streaming API.
#[derive(Deserialize)]
struct ClaudeSSEEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<ClaudeDelta>,
}

#[derive(Deserialize)]
struct ClaudeDelta {
    text: Option<String>,
}

#[async_trait]
impl AiProvider for ClaudeProvider {
    async fn complete(&self, messages: &[ChatMessage], max_tokens: Option<u32>) -> Result<String, AppError> {
        let (system, claude_messages) = self.build_request(messages, false);

        let body = ClaudeRequest {
            model: self.model.clone(),
            max_tokens: max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
            system,
            messages: claude_messages,
            stream: false,
        };

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::AiProviderError(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "could not read body".into());
            return Err(AppError::AiProviderError(format!(
                "Anthropic API error ({}): {}",
                status, text
            )));
        }

        let resp: ClaudeResponse = response
            .json()
            .await
            .map_err(|e| AppError::AiProviderError(format!("Failed to parse response: {}", e)))?;

        let text = resp
            .content
            .first()
            .and_then(|b| b.text.clone())
            .unwrap_or_default();

        Ok(text)
    }

    async fn stream(
        &self,
        messages: &[ChatMessage],
        tx: mpsc::Sender<StreamDelta>,
    ) -> Result<(), AppError> {
        let (system, claude_messages) = self.build_request(messages, true);

        let body = ClaudeRequest {
            model: self.model.clone(),
            max_tokens: DEFAULT_MAX_TOKENS,
            system,
            messages: claude_messages,
            stream: true,
        };

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::AiProviderError(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "could not read body".into());
            return Err(AppError::AiProviderError(format!(
                "Anthropic API error ({}): {}",
                status, text
            )));
        }

        let mut byte_stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_result) = byte_stream.next().await {
            let chunk = chunk_result.map_err(|e| {
                AppError::AiProviderError(format!("Stream read error: {}", e))
            })?;

            let chunk_str = String::from_utf8_lossy(&chunk);
            buffer.push_str(&chunk_str);

            // Process complete SSE events (separated by double newlines).
            while let Some(pos) = buffer.find("\n\n") {
                let event_block = buffer[..pos].to_string();
                buffer = buffer[pos + 2..].to_string();

                let events = parse_sse_events(&event_block);
                for event_data in events {
                    if let Ok(sse_event) = serde_json::from_str::<ClaudeSSEEvent>(&event_data) {
                        match sse_event.event_type.as_str() {
                            "content_block_delta" => {
                                if let Some(delta) = sse_event.delta {
                                    if let Some(text) = delta.text {
                                        let _ = tx
                                            .send(StreamDelta {
                                                content: text,
                                                done: false,
                                            })
                                            .await;
                                    }
                                }
                            }
                            "message_stop" => {
                                let _ = tx
                                    .send(StreamDelta {
                                        content: String::new(),
                                        done: true,
                                    })
                                    .await;
                            }
                            _ => {
                                // Ignore other event types (message_start, content_block_start, etc.)
                            }
                        }
                    }
                }
            }
        }

        // Ensure a done signal was sent even if the stream ended without message_stop.
        let _ = tx
            .send(StreamDelta {
                content: String::new(),
                done: true,
            })
            .await;

        Ok(())
    }
}
