use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::ai::message::ChatMessage;
use crate::error::AppError;

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
    async fn complete(&self, messages: &[ChatMessage]) -> Result<String, AppError>;

    /// Send messages and stream the response via a channel.
    async fn stream(
        &self,
        messages: &[ChatMessage],
        tx: mpsc::Sender<StreamDelta>,
    ) -> Result<(), AppError>;
}
