use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;
use tokio::sync::mpsc;

use crate::ai::claude::ClaudeProvider;
use crate::ai::gemini::GeminiProvider;
use crate::ai::message::ChatMessage;
use crate::ai::ollama::OllamaProvider;
use crate::ai::openai::OpenAiProvider;
use crate::ai::provider::{AiProvider, StreamDelta};
use crate::agent::prompts;
use crate::agent::validate;
use crate::config::AppConfig;
use crate::error::AppError;
use crate::state::AppState;

/// Event payload sent to the frontend over a Tauri Channel during streaming.
#[derive(Clone, Serialize)]
pub struct StreamEvent {
    pub delta: String,
    pub done: bool,
}

/// Result returned by the auto_retry command.
#[derive(Clone, Serialize)]
pub struct AutoRetryResult {
    pub new_code: Option<String>,
    pub ai_response: String,
    pub attempt: u32,
    pub success: bool,
}

/// Create an AI provider based on the current configuration.
/// Shared between `send_message` and `auto_retry` to avoid code duplication.
fn create_provider(config: &AppConfig) -> Result<Box<dyn AiProvider>, AppError> {
    match config.ai_provider.as_str() {
        "openai" => {
            let api_key = config
                .api_key
                .clone()
                .ok_or_else(|| AppError::AiProviderError("OpenAI API key not set".into()))?;
            Ok(Box::new(OpenAiProvider::new(
                api_key,
                config.model.clone(),
                config.openai_base_url.clone(),
            )))
        }
        "deepseek" => {
            let api_key = config
                .api_key
                .clone()
                .ok_or_else(|| AppError::AiProviderError("DeepSeek API key not set".into()))?;
            Ok(Box::new(OpenAiProvider::new(
                api_key,
                config.model.clone(),
                Some("https://api.deepseek.com/v1".to_string()),
            )))
        }
        "qwen" => {
            let api_key = config
                .api_key
                .clone()
                .ok_or_else(|| AppError::AiProviderError("Qwen API key not set".into()))?;
            Ok(Box::new(OpenAiProvider::new(
                api_key,
                config.model.clone(),
                Some("https://dashscope-intl.aliyuncs.com/compatible-mode/v1".to_string()),
            )))
        }
        "kimi" => {
            let api_key = config
                .api_key
                .clone()
                .ok_or_else(|| AppError::AiProviderError("Kimi API key not set".into()))?;
            Ok(Box::new(OpenAiProvider::new(
                api_key,
                config.model.clone(),
                Some("https://api.moonshot.ai/v1".to_string()),
            )))
        }
        "gemini" => {
            let api_key = config
                .api_key
                .clone()
                .ok_or_else(|| AppError::AiProviderError("Gemini API key not set".into()))?;
            Ok(Box::new(GeminiProvider::new(
                api_key,
                config.model.clone(),
            )))
        }
        "ollama" => Ok(Box::new(OllamaProvider::new(
            config.ollama_base_url.clone(),
            config.model.clone(),
        ))),
        _ => {
            // Default to Claude.
            let api_key = config
                .api_key
                .clone()
                .ok_or_else(|| AppError::AiProviderError("API key not set".into()))?;
            Ok(Box::new(ClaudeProvider::new(
                api_key,
                config.model.clone(),
            )))
        }
    }
}

/// Stream an AI response from the provider, forwarding deltas over the Tauri channel.
/// Returns the fully accumulated response string.
async fn stream_ai_response(
    provider: Box<dyn AiProvider>,
    messages: Vec<ChatMessage>,
    on_event: &Channel<StreamEvent>,
) -> Result<String, AppError> {
    let (tx, mut rx) = mpsc::channel::<StreamDelta>(100);

    let provider_handle = tokio::spawn(async move { provider.stream(&messages, tx).await });

    let mut full_response = String::new();

    while let Some(delta) = rx.recv().await {
        full_response.push_str(&delta.content);
        let _ = on_event.send(StreamEvent {
            delta: delta.content,
            done: delta.done,
        });
    }

    match provider_handle.await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(e),
        Err(e) => {
            return Err(AppError::AiProviderError(format!(
                "Provider task panicked: {}",
                e
            )));
        }
    }

    Ok(full_response)
}

#[tauri::command]
pub async fn send_message(
    message: String,
    history: Vec<ChatMessage>,
    on_event: Channel<StreamEvent>,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    // Read config (clone to release the lock immediately).
    let config = state.config.lock().unwrap().clone();

    // Build the system prompt from default rules.
    let system_prompt = prompts::build_default_system_prompt();

    // Create the AI provider.
    let provider = create_provider(&config)?;

    // Build the full message list: system prompt + conversation history + new user message.
    let mut messages = vec![ChatMessage {
        role: "system".to_string(),
        content: system_prompt,
    }];
    messages.extend(history);
    messages.push(ChatMessage {
        role: "user".to_string(),
        content: message,
    });

    // Stream and return the full response.
    stream_ai_response(provider, messages, &on_event).await
}

#[tauri::command]
pub async fn auto_retry(
    failed_code: String,
    error_message: String,
    history: Vec<ChatMessage>,
    attempt: u32,
    on_event: Channel<StreamEvent>,
    state: State<'_, AppState>,
) -> Result<AutoRetryResult, AppError> {
    // Read config.
    let config = state.config.lock().unwrap().clone();

    // Build the system prompt.
    let system_prompt = prompts::build_default_system_prompt();

    // Create the AI provider.
    let provider = create_provider(&config)?;

    // Build a retry message that includes the failed code and error.
    let retry_message = format!(
        "The following CadQuery code failed with an error. Please fix it and provide the complete corrected code.\n\n\
        Code that failed:\n```python\n{}\n```\n\n\
        Error:\n```\n{}\n```\n\n\
        Please provide the complete corrected code in a ```python block. Only output the fixed code, do not repeat the error.",
        failed_code, error_message
    );

    // Build message list: system + history + retry request.
    let mut messages = vec![ChatMessage {
        role: "system".to_string(),
        content: system_prompt,
    }];
    messages.extend(history);
    messages.push(ChatMessage {
        role: "user".to_string(),
        content: retry_message,
    });

    // Stream the AI response.
    let full_response = stream_ai_response(provider, messages, &on_event).await?;

    // Extract code from AI response.
    let code = validate::extract_python_code(&full_response);

    // If code was extracted, validate it has basic CadQuery structure.
    let valid_code = code.and_then(|c| {
        match validate::validate_cadquery_code(&c) {
            Ok(()) => Some(c),
            Err(_) => {
                // Code extracted but doesn't pass basic validation.
                // Still return it — the execution attempt will surface the real error.
                Some(c)
            }
        }
    });

    Ok(AutoRetryResult {
        success: valid_code.is_some(),
        new_code: valid_code,
        ai_response: full_response,
        attempt,
    })
}
