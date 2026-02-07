use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::ai::message::ChatMessage;
use crate::ai::provider::{AiProvider, StreamDelta};
use crate::ai::streaming::parse_sse_events;
use crate::error::AppError;

const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

pub struct GeminiProvider {
    client: Client,
    api_key: String,
    model: String,
}

#[allow(dead_code)]
impl GeminiProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
        }
    }

    fn generate_endpoint(&self) -> String {
        format!(
            "{}/models/{}:generateContent?key={}",
            GEMINI_API_BASE, self.model, self.api_key
        )
    }

    fn stream_endpoint(&self) -> String {
        format!(
            "{}/models/{}:streamGenerateContent?alt=sse&key={}",
            GEMINI_API_BASE, self.model, self.api_key
        )
    }

    /// Build a Gemini request body from ChatMessages.
    /// Separates system messages into `system_instruction` and maps roles.
    fn build_request(&self, messages: &[ChatMessage]) -> GeminiRequest {
        let mut system_parts: Vec<GeminiPart> = Vec::new();
        let mut contents: Vec<GeminiContent> = Vec::new();

        for msg in messages {
            if msg.role == "system" {
                system_parts.push(GeminiPart {
                    text: msg.content.clone(),
                });
            } else {
                let role = match msg.role.as_str() {
                    "assistant" => "model".to_string(),
                    other => other.to_string(),
                };
                contents.push(GeminiContent {
                    role,
                    parts: vec![GeminiPart {
                        text: msg.content.clone(),
                    }],
                });
            }
        }

        let system_instruction = if system_parts.is_empty() {
            None
        } else {
            Some(GeminiSystemInstruction {
                parts: system_parts,
            })
        };

        GeminiRequest {
            contents,
            system_instruction,
        }
    }
}

// --- Request types ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiSystemInstruction>,
}

#[derive(Serialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Serialize)]
struct GeminiSystemInstruction {
    parts: Vec<GeminiPart>,
}

// --- Response types ---

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiResponseContent>,
}

#[derive(Deserialize)]
struct GeminiResponseContent {
    parts: Option<Vec<GeminiResponsePart>>,
}

#[derive(Deserialize)]
struct GeminiResponsePart {
    text: Option<String>,
}

#[async_trait]
impl AiProvider for GeminiProvider {
    async fn complete(&self, messages: &[ChatMessage]) -> Result<String, AppError> {
        let body = self.build_request(messages);

        let response = self
            .client
            .post(&self.generate_endpoint())
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
                "Gemini API error ({}): {}",
                status, text
            )));
        }

        let resp: GeminiResponse = response
            .json()
            .await
            .map_err(|e| AppError::AiProviderError(format!("Failed to parse response: {}", e)))?;

        let text = resp
            .candidates
            .as_ref()
            .and_then(|c| c.first())
            .and_then(|c| c.content.as_ref())
            .and_then(|c| c.parts.as_ref())
            .and_then(|p| p.first())
            .and_then(|p| p.text.clone())
            .unwrap_or_default();

        Ok(text)
    }

    async fn stream(
        &self,
        messages: &[ChatMessage],
        tx: mpsc::Sender<StreamDelta>,
    ) -> Result<(), AppError> {
        let body = self.build_request(messages);

        let response = self
            .client
            .post(&self.stream_endpoint())
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
                "Gemini API error ({}): {}",
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

            // Process complete SSE events.
            while let Some(pos) = buffer.find("\n\n") {
                let event_block = buffer[..pos].to_string();
                buffer = buffer[pos + 2..].to_string();

                let events = parse_sse_events(&event_block);
                for event_data in events {
                    if let Ok(resp) = serde_json::from_str::<GeminiResponse>(&event_data) {
                        let text = resp
                            .candidates
                            .as_ref()
                            .and_then(|c| c.first())
                            .and_then(|c| c.content.as_ref())
                            .and_then(|c| c.parts.as_ref())
                            .and_then(|p| p.first())
                            .and_then(|p| p.text.clone());

                        if let Some(text) = text {
                            let _ = tx
                                .send(StreamDelta {
                                    content: text,
                                    done: false,
                                })
                                .await;
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

        Ok(())
    }
}
