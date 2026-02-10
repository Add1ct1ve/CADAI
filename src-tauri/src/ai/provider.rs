use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::ai::message::ChatMessage;
use crate::error::AppError;

/// Token usage from an AI provider call.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

impl TokenUsage {
    pub fn total(&self) -> u32 {
        self.input_tokens + self.output_tokens
    }
    pub fn add(&mut self, other: &TokenUsage) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
    }
}

/// A streaming delta from the AI provider.
#[derive(Debug, Clone)]
pub struct StreamDelta {
    pub content: String,
    pub done: bool,
}

#[async_trait]
#[allow(dead_code)]
pub trait AiProvider: Send + Sync {
    /// Send messages and get a complete response.
    /// If `max_tokens` is `Some(n)`, cap the response length; otherwise use the provider default.
    async fn complete(&self, messages: &[ChatMessage], max_tokens: Option<u32>) -> Result<(String, Option<TokenUsage>), AppError>;

    /// Send messages and stream the response via a channel.
    async fn stream(
        &self,
        messages: &[ChatMessage],
        tx: mpsc::Sender<StreamDelta>,
    ) -> Result<Option<TokenUsage>, AppError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_usage_default() {
        let usage = TokenUsage::default();
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
    }

    #[test]
    fn test_token_usage_total() {
        let usage = TokenUsage { input_tokens: 100, output_tokens: 50 };
        assert_eq!(usage.total(), 150);
    }

    #[test]
    fn test_token_usage_add() {
        let mut usage = TokenUsage { input_tokens: 100, output_tokens: 50 };
        let other = TokenUsage { input_tokens: 200, output_tokens: 75 };
        usage.add(&other);
        assert_eq!(usage.input_tokens, 300);
        assert_eq!(usage.output_tokens, 125);
        assert_eq!(usage.total(), 425);
    }
}
