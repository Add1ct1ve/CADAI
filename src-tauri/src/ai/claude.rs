use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::ai::message::ChatMessage;
use crate::ai::provider::{AiProvider, StreamDelta, TokenUsage};
use crate::ai::streaming::parse_sse_events;
use crate::error::AppError;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MAX_TOKENS: u32 = 4096;

pub struct ClaudeProvider {
    client: Client,
    api_key: String,
    model: String,
    temperature: Option<f32>,
}

impl ClaudeProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
            temperature: None,
        }
    }

    pub fn with_temperature(mut self, temperature: Option<f32>) -> Self {
        self.temperature = temperature;
        self
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
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
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
    usage: Option<ClaudeUsage>,
}

#[derive(Deserialize)]
struct ClaudeUsage {
    input_tokens: u32,
    output_tokens: u32,
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
    message: Option<ClaudeMessageStart>,
    usage: Option<ClaudeStreamEndUsage>,
}

#[derive(Deserialize)]
struct ClaudeDelta {
    text: Option<String>,
}

#[derive(Deserialize)]
struct ClaudeMessageStart {
    usage: Option<ClaudeUsage>,
}

#[derive(Deserialize)]
struct ClaudeStreamEndUsage {
    output_tokens: Option<u32>,
}

#[async_trait]
impl AiProvider for ClaudeProvider {
    async fn complete(&self, messages: &[ChatMessage], max_tokens: Option<u32>) -> Result<(String, Option<TokenUsage>), AppError> {
        let (system, claude_messages) = self.build_request(messages, false);

        let body = ClaudeRequest {
            model: self.model.clone(),
            max_tokens: max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
            system,
            messages: claude_messages,
            stream: false,
            temperature: self.temperature,
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

        let usage = resp.usage.map(|u| TokenUsage {
            input_tokens: u.input_tokens,
            output_tokens: u.output_tokens,
        });

        Ok((text, usage))
    }

    async fn stream(
        &self,
        messages: &[ChatMessage],
        tx: mpsc::Sender<StreamDelta>,
    ) -> Result<Option<TokenUsage>, AppError> {
        let (system, claude_messages) = self.build_request(messages, true);

        let body = ClaudeRequest {
            model: self.model.clone(),
            max_tokens: DEFAULT_MAX_TOKENS,
            system,
            messages: claude_messages,
            stream: true,
            temperature: self.temperature,
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
        let mut tracked_usage = TokenUsage::default();
        let mut has_usage = false;

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
                            "message_start" => {
                                // Capture input_tokens from message_start event
                                if let Some(msg) = sse_event.message {
                                    if let Some(u) = msg.usage {
                                        tracked_usage.input_tokens = u.input_tokens;
                                        has_usage = true;
                                    }
                                }
                            }
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
                            "message_delta" => {
                                // Capture output_tokens from message_delta event
                                if let Some(u) = sse_event.usage {
                                    if let Some(output) = u.output_tokens {
                                        tracked_usage.output_tokens = output;
                                        has_usage = true;
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
                                // Ignore other event types (content_block_start, etc.)
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

        Ok(if has_usage { Some(tracked_usage) } else { None })
    }
}
