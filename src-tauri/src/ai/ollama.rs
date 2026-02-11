use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::ai::message::ChatMessage;
use crate::ai::provider::{AiProvider, StreamDelta, TokenUsage};
use crate::error::AppError;

const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

pub struct OllamaProvider {
    client: Client,
    base_url: String,
    model: String,
    temperature: Option<f32>,
}

impl OllamaProvider {
    pub fn new(base_url: Option<String>, model: String) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.unwrap_or_else(|| DEFAULT_OLLAMA_URL.to_string()),
            model,
            temperature: None,
        }
    }

    pub fn with_temperature(mut self, temperature: Option<f32>) -> Self {
        self.temperature = temperature;
        self
    }

    fn chat_endpoint(&self) -> String {
        format!("{}/api/chat", self.base_url)
    }
}

// --- Request / Response types for the Ollama Chat API ---

#[derive(Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

/// Non-streaming response from Ollama.
#[derive(Deserialize)]
#[allow(dead_code)]
struct OllamaResponse {
    message: Option<OllamaResponseMessage>,
    prompt_eval_count: Option<u32>,
    eval_count: Option<u32>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OllamaResponseMessage {
    content: Option<String>,
}

/// Each line in NDJSON streaming from Ollama.
#[derive(Deserialize)]
struct OllamaStreamLine {
    message: Option<OllamaStreamMessage>,
    done: Option<bool>,
    prompt_eval_count: Option<u32>,
    eval_count: Option<u32>,
}

#[derive(Deserialize)]
struct OllamaStreamMessage {
    content: Option<String>,
}

impl From<&ChatMessage> for OllamaMessage {
    fn from(msg: &ChatMessage) -> Self {
        Self {
            role: msg.role.clone(),
            content: msg.content.clone(),
        }
    }
}

#[async_trait]
impl AiProvider for OllamaProvider {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        _max_tokens: Option<u32>,
    ) -> Result<(String, Option<TokenUsage>), AppError> {
        let ollama_messages: Vec<OllamaMessage> =
            messages.iter().map(OllamaMessage::from).collect();

        let options = self.temperature.map(|t| OllamaOptions {
            temperature: Some(t),
        });

        let body = OllamaRequest {
            model: self.model.clone(),
            messages: ollama_messages,
            stream: false,
            options,
        };

        let response = self
            .client
            .post(&self.chat_endpoint())
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
                "Ollama API error ({}): {}",
                status, text
            )));
        }

        let resp: OllamaResponse = response
            .json()
            .await
            .map_err(|e| AppError::AiProviderError(format!("Failed to parse response: {}", e)))?;

        let text = resp.message.and_then(|m| m.content).unwrap_or_default();

        if text.is_empty() {
            eprintln!(
                "[ollama] Warning: API returned empty text. Model: {}",
                self.model
            );
        }

        let usage = match (resp.prompt_eval_count, resp.eval_count) {
            (Some(input), Some(output)) => Some(TokenUsage {
                input_tokens: input,
                output_tokens: output,
            }),
            _ => None,
        };

        Ok((text, usage))
    }

    async fn stream(
        &self,
        messages: &[ChatMessage],
        tx: mpsc::Sender<StreamDelta>,
    ) -> Result<Option<TokenUsage>, AppError> {
        let ollama_messages: Vec<OllamaMessage> =
            messages.iter().map(OllamaMessage::from).collect();

        let options = self.temperature.map(|t| OllamaOptions {
            temperature: Some(t),
        });

        let body = OllamaRequest {
            model: self.model.clone(),
            messages: ollama_messages,
            stream: true,
            options,
        };

        let response = self
            .client
            .post(&self.chat_endpoint())
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
                "Ollama API error ({}): {}",
                status, text
            )));
        }

        // Ollama uses NDJSON: each line is a complete JSON object.
        let mut byte_stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut tracked_usage: Option<TokenUsage> = None;

        while let Some(chunk_result) = byte_stream.next().await {
            let chunk = chunk_result
                .map_err(|e| AppError::AiProviderError(format!("Stream read error: {}", e)))?;

            let chunk_str = String::from_utf8_lossy(&chunk);
            buffer.push_str(&chunk_str);

            // Process complete lines (NDJSON = one JSON object per line).
            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                if let Ok(stream_line) = serde_json::from_str::<OllamaStreamLine>(&line) {
                    let is_done = stream_line.done.unwrap_or(false);
                    let content = stream_line
                        .message
                        .and_then(|m| m.content)
                        .unwrap_or_default();

                    // Capture usage from the done line
                    if is_done {
                        if let (Some(input), Some(output)) =
                            (stream_line.prompt_eval_count, stream_line.eval_count)
                        {
                            tracked_usage = Some(TokenUsage {
                                input_tokens: input,
                                output_tokens: output,
                            });
                        }
                    }

                    let _ = tx
                        .send(StreamDelta {
                            content,
                            done: is_done,
                        })
                        .await;

                    if is_done {
                        return Ok(tracked_usage);
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
