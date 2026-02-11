use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::ai::message::ChatMessage;
use crate::ai::provider::{AiProvider, StreamDelta, TokenUsage};
use crate::ai::streaming::parse_sse_events;
use crate::error::AppError;

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

pub struct OpenAiProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
    temperature: Option<f32>,
}

impl OpenAiProvider {
    pub fn new(api_key: String, model: String, base_url: Option<String>) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
            temperature: None,
        }
    }

    pub fn with_temperature(mut self, temperature: Option<f32>) -> Self {
        self.temperature = temperature;
        self
    }

    fn chat_endpoint(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }
}

// --- Request / Response types for the OpenAI Chat Completions API ---

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_options: Option<OpenAiStreamOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Serialize)]
struct OpenAiStreamOptions {
    include_usage: bool,
}

#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OpenAiChoice {
    message: Option<OpenAiMessageContent>,
    delta: Option<OpenAiDeltaContent>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OpenAiMessageContent {
    content: Option<String>,
    /// Thinking/reasoning models (Kimi K2.5, DeepSeek R1, etc.) put their
    /// chain-of-thought here. When `content` is empty, fall back to this.
    reasoning_content: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiDeltaContent {
    content: Option<String>,
    /// Thinking models stream reasoning here before the final content.
    reasoning_content: Option<String>,
}

/// SSE chunk from the OpenAI streaming API.
#[derive(Deserialize)]
struct OpenAiStreamChunk {
    choices: Vec<OpenAiStreamChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Deserialize)]
struct OpenAiStreamChoice {
    delta: Option<OpenAiDeltaContent>,
    finish_reason: Option<String>,
}

impl From<&ChatMessage> for OpenAiMessage {
    fn from(msg: &ChatMessage) -> Self {
        Self {
            role: msg.role.clone(),
            content: msg.content.clone(),
        }
    }
}

#[async_trait]
impl AiProvider for OpenAiProvider {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        max_tokens: Option<u32>,
    ) -> Result<(String, Option<TokenUsage>), AppError> {
        let openai_messages: Vec<OpenAiMessage> =
            messages.iter().map(OpenAiMessage::from).collect();

        let body = OpenAiRequest {
            model: self.model.clone(),
            messages: openai_messages,
            stream: false,
            max_tokens,
            stream_options: None,
            temperature: self.temperature,
        };

        let response = self
            .client
            .post(&self.chat_endpoint())
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
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
                "OpenAI API error ({}): {}",
                status, text
            )));
        }

        let resp: OpenAiResponse = response
            .json()
            .await
            .map_err(|e| AppError::AiProviderError(format!("Failed to parse response: {}", e)))?;

        let message = resp.choices.first().and_then(|c| c.message.as_ref());
        // Prefer `content`; fall back to `reasoning_content` for thinking models
        // (Kimi K2.5, DeepSeek R1, etc.) that put output there instead.
        let text = message
            .and_then(|m| {
                m.content
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .or(m.reasoning_content.as_deref())
            })
            .unwrap_or_default()
            .to_string();

        if text.is_empty() {
            eprintln!(
                "[openai] Warning: API returned empty text. Choices: {}, model: {}",
                resp.choices.len(),
                self.model
            );
        }

        let usage = resp.usage.map(|u| TokenUsage {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
        });

        Ok((text, usage))
    }

    async fn stream(
        &self,
        messages: &[ChatMessage],
        tx: mpsc::Sender<StreamDelta>,
    ) -> Result<Option<TokenUsage>, AppError> {
        let openai_messages: Vec<OpenAiMessage> =
            messages.iter().map(OpenAiMessage::from).collect();

        let body = OpenAiRequest {
            model: self.model.clone(),
            messages: openai_messages,
            stream: true,
            max_tokens: None,
            stream_options: Some(OpenAiStreamOptions {
                include_usage: true,
            }),
            temperature: self.temperature,
        };

        let response = self
            .client
            .post(&self.chat_endpoint())
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
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
                "OpenAI API error ({}): {}",
                status, text
            )));
        }

        let mut byte_stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut tracked_usage: Option<TokenUsage> = None;

        while let Some(chunk_result) = byte_stream.next().await {
            let chunk = chunk_result
                .map_err(|e| AppError::AiProviderError(format!("Stream read error: {}", e)))?;

            let chunk_str = String::from_utf8_lossy(&chunk);
            buffer.push_str(&chunk_str);

            // Process complete SSE events.
            while let Some(pos) = buffer.find("\n\n") {
                let event_block = buffer[..pos].to_string();
                buffer = buffer[pos + 2..].to_string();

                let events = parse_sse_events(&event_block);
                for event_data in events {
                    if let Ok(chunk) = serde_json::from_str::<OpenAiStreamChunk>(&event_data) {
                        // Capture usage from the final chunk
                        if let Some(u) = &chunk.usage {
                            tracked_usage = Some(TokenUsage {
                                input_tokens: u.prompt_tokens,
                                output_tokens: u.completion_tokens,
                            });
                        }
                        for choice in &chunk.choices {
                            if let Some(delta) = &choice.delta {
                                // Emit content deltas (standard models)
                                if let Some(ref content) = delta.content {
                                    let _ = tx
                                        .send(StreamDelta {
                                            content: content.clone(),
                                            done: false,
                                        })
                                        .await;
                                }
                                // Emit reasoning_content deltas as regular content
                                // for thinking models (Kimi K2.5, DeepSeek R1, etc.)
                                // only when there's no regular content in this chunk.
                                if delta.content.is_none() {
                                    if let Some(ref reasoning) = delta.reasoning_content {
                                        let _ = tx
                                            .send(StreamDelta {
                                                content: reasoning.clone(),
                                                done: false,
                                            })
                                            .await;
                                    }
                                }
                            }
                            if choice.finish_reason.is_some() {
                                let _ = tx
                                    .send(StreamDelta {
                                        content: String::new(),
                                        done: true,
                                    })
                                    .await;
                            }
                        }
                    }
                }
            }
        }

        // Ensure a done signal is always sent.
        let _ = tx
            .send(StreamDelta {
                content: String::new(),
                done: true,
            })
            .await;

        Ok(tracked_usage)
    }
}
