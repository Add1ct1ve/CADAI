use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;
use tokio::sync::mpsc;

use crate::ai::claude::ClaudeProvider;
use crate::ai::gemini::GeminiProvider;
use crate::ai::message::ChatMessage;
use crate::ai::ollama::OllamaProvider;
use crate::ai::openai::OpenAiProvider;
use crate::ai::cost;
use crate::ai::provider::{AiProvider, StreamDelta, TokenUsage};
use crate::agent::prompts;
use crate::agent::rules::{AgentRules, AntiPatternEntry};
use crate::agent::validate;
use crate::config::AppConfig;
use crate::error::AppError;
use crate::state::AppState;

/// Event payload sent to the frontend over a Tauri Channel during streaming.
#[derive(Clone, Serialize)]
pub struct StreamEvent {
    pub delta: String,
    pub done: bool,
    /// Optional event type: "design_plan" for geometry plans, "token_usage" for usage data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<StreamTokenUsage>,
}

#[derive(Clone, Serialize)]
pub struct StreamTokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
    pub cost_usd: Option<f64>,
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
/// Shared between `send_message`, `auto_retry`, and `generate_parallel`.
pub(crate) fn create_provider(config: &AppConfig) -> Result<Box<dyn AiProvider>, AppError> {
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

/// Create an AI provider with an explicit temperature setting.
/// Used by consensus mode to run parallel generations at different temperatures.
pub(crate) fn create_provider_with_temp(
    config: &AppConfig,
    temperature: Option<f32>,
) -> Result<Box<dyn AiProvider>, AppError> {
    match config.ai_provider.as_str() {
        "openai" => {
            let api_key = config
                .api_key
                .clone()
                .ok_or_else(|| AppError::AiProviderError("OpenAI API key not set".into()))?;
            Ok(Box::new(
                OpenAiProvider::new(api_key, config.model.clone(), config.openai_base_url.clone())
                    .with_temperature(temperature),
            ))
        }
        "deepseek" => {
            let api_key = config
                .api_key
                .clone()
                .ok_or_else(|| AppError::AiProviderError("DeepSeek API key not set".into()))?;
            Ok(Box::new(
                OpenAiProvider::new(
                    api_key,
                    config.model.clone(),
                    Some("https://api.deepseek.com/v1".to_string()),
                )
                .with_temperature(temperature),
            ))
        }
        "qwen" => {
            let api_key = config
                .api_key
                .clone()
                .ok_or_else(|| AppError::AiProviderError("Qwen API key not set".into()))?;
            Ok(Box::new(
                OpenAiProvider::new(
                    api_key,
                    config.model.clone(),
                    Some("https://dashscope-intl.aliyuncs.com/compatible-mode/v1".to_string()),
                )
                .with_temperature(temperature),
            ))
        }
        "kimi" => {
            let api_key = config
                .api_key
                .clone()
                .ok_or_else(|| AppError::AiProviderError("Kimi API key not set".into()))?;
            Ok(Box::new(
                OpenAiProvider::new(
                    api_key,
                    config.model.clone(),
                    Some("https://api.moonshot.ai/v1".to_string()),
                )
                .with_temperature(temperature),
            ))
        }
        "gemini" => {
            let api_key = config
                .api_key
                .clone()
                .ok_or_else(|| AppError::AiProviderError("Gemini API key not set".into()))?;
            Ok(Box::new(
                GeminiProvider::new(api_key, config.model.clone())
                    .with_temperature(temperature),
            ))
        }
        "ollama" => Ok(Box::new(
            OllamaProvider::new(config.ollama_base_url.clone(), config.model.clone())
                .with_temperature(temperature),
        )),
        _ => {
            let api_key = config
                .api_key
                .clone()
                .ok_or_else(|| AppError::AiProviderError("API key not set".into()))?;
            Ok(Box::new(
                ClaudeProvider::new(api_key, config.model.clone())
                    .with_temperature(temperature),
            ))
        }
    }
}

/// Stream an AI response from the provider, forwarding deltas over the Tauri channel.
/// Returns the fully accumulated response string and optional token usage.
pub(crate) async fn stream_ai_response(
    provider: Box<dyn AiProvider>,
    messages: Vec<ChatMessage>,
    on_event: &Channel<StreamEvent>,
) -> Result<(String, Option<TokenUsage>), AppError> {
    let (tx, mut rx) = mpsc::channel::<StreamDelta>(100);

    let provider_handle = tokio::spawn(async move { provider.stream(&messages, tx).await });

    let mut full_response = String::new();

    while let Some(delta) = rx.recv().await {
        full_response.push_str(&delta.content);
        let _ = on_event.send(StreamEvent {
            delta: delta.content,
            done: delta.done,
            event_type: None,
            token_usage: None,
        });
    }

    let usage = match provider_handle.await {
        Ok(Ok(usage)) => usage,
        Ok(Err(e)) => return Err(e),
        Err(e) => {
            return Err(AppError::AiProviderError(format!(
                "Provider task panicked: {}",
                e
            )));
        }
    };

    Ok((full_response, usage))
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

    // Build the system prompt from the configured preset.
    let cq_version = state.cadquery_version.lock().unwrap().clone();
    let system_prompt = prompts::build_system_prompt_for_preset(
        config.agent_rules_preset.as_deref(),
        cq_version.as_deref(),
    );

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
    let (full_response, usage) = stream_ai_response(provider, messages, &on_event).await?;

    // Emit token usage event if available.
    if let Some(ref u) = usage {
        let cost_usd = cost::estimate_cost(&config.ai_provider, &config.model, u);
        let _ = on_event.send(StreamEvent {
            delta: String::new(),
            done: true,
            event_type: Some("token_usage".to_string()),
            token_usage: Some(StreamTokenUsage {
                input_tokens: u.input_tokens,
                output_tokens: u.output_tokens,
                total_tokens: u.total(),
                cost_usd,
            }),
        });
    }

    Ok(full_response)
}

/// Build a targeted retry prompt from the error classification and strategy.
///
/// Assembles the failed code, error message, fix instruction, anti-pattern hint,
/// forbidden operations, and error context into a single prompt for the AI.
pub(crate) fn build_retry_prompt(
    failed_code: &str,
    error_message: &str,
    error: &validate::StructuredError,
    strategy: &validate::RetryStrategy,
    anti_pattern: Option<&AntiPatternEntry>,
) -> String {
    let mut prompt = String::new();

    prompt.push_str("The following CadQuery code failed with an error. Please fix it.\n\n");
    prompt.push_str(&format!(
        "Code that failed:\n```python\n{}\n```\n\n",
        failed_code
    ));
    prompt.push_str(&format!("Error:\n```\n{}\n```\n\n", error_message));

    // Primary fix instruction from the strategy.
    prompt.push_str(&format!(
        "**Fix instruction:** {}\n\n",
        strategy.fix_instruction
    ));

    // Include anti-pattern guidance if available.
    if let Some(ap) = anti_pattern {
        prompt.push_str(&format!(
            "**Known anti-pattern — {}:** {}\nCorrect approach:\n```python\n{}\n```\n\n",
            ap.title, ap.explanation, ap.correct_code
        ));
    }

    // Include forbidden operations.
    if !strategy.forbidden_operations.is_empty() {
        prompt.push_str(&format!(
            "Do NOT use these operations: {}\n\n",
            strategy.forbidden_operations.join(", ")
        ));
    }

    // Include source line context if available.
    if let Some(ref ctx) = error.context {
        if let Some(ref source_line) = ctx.source_line {
            prompt.push_str(&format!("The failing line: `{}`\n\n", source_line));
        }
    }

    // Include failing operation if known.
    if let Some(ref op) = error.failing_operation {
        prompt.push_str(&format!("The failing operation: `{}`\n\n", op));
    }

    prompt.push_str("Provide the complete corrected code wrapped in <CODE>...</CODE> tags.");

    prompt
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

    // Build the system prompt from the configured preset.
    let cq_version = state.cadquery_version.lock().unwrap().clone();
    let system_prompt = prompts::build_system_prompt_for_preset(
        config.agent_rules_preset.as_deref(),
        cq_version.as_deref(),
    );

    // Create the AI provider.
    let provider = create_provider(&config)?;

    // Classify the error and build a targeted retry prompt.
    let structured_error = validate::parse_traceback(&error_message);
    let strategy = validate::get_retry_strategy(&structured_error, attempt);

    // Look up matching anti-pattern from the agent rules (if any).
    let rules = AgentRules::from_preset(config.agent_rules_preset.as_deref()).ok();
    let anti_pattern = rules.as_ref().and_then(|r| {
        r.anti_patterns.as_ref().and_then(|patterns| {
            strategy.matching_anti_pattern.as_ref().and_then(|title| {
                patterns.iter().find(|p| p.title == *title)
            })
        })
    });

    let retry_message = build_retry_prompt(
        &failed_code,
        &error_message,
        &structured_error,
        &strategy,
        anti_pattern,
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
    let (full_response, usage) = stream_ai_response(provider, messages, &on_event).await?;

    // Emit token usage event if available.
    if let Some(ref u) = usage {
        let cost_usd = cost::estimate_cost(&config.ai_provider, &config.model, u);
        let _ = on_event.send(StreamEvent {
            delta: String::new(),
            done: true,
            event_type: Some("token_usage".to_string()),
            token_usage: Some(StreamTokenUsage {
                input_tokens: u.input_tokens,
                output_tokens: u.output_tokens,
                total_tokens: u.total(),
                cost_usd,
            }),
        });
    }

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

#[tauri::command]
pub fn clear_session_memory(state: State<'_, AppState>) -> Result<(), AppError> {
    state.session_memory.lock().unwrap().reset();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::validate::{
        ErrorCategory, ErrorContext, RetryStrategy, StructuredError, TopologySubKind,
    };

    fn make_test_error() -> StructuredError {
        StructuredError {
            error_type: "OCP.StdFail_NotDone".to_string(),
            message: "BRep_API: not done".to_string(),
            line_number: Some(6),
            suggestion: None,
            category: ErrorCategory::Topology(TopologySubKind::FilletFailure),
            failing_operation: Some("fillet".to_string()),
            context: Some(ErrorContext {
                source_line: Some("result = base.fillet(8.0)".to_string()),
                failing_parameters: Some("8.0".to_string()),
            }),
        }
    }

    fn make_test_strategy() -> RetryStrategy {
        RetryStrategy {
            fix_instruction: "Reduce ALL fillet radii by 50%.".to_string(),
            forbidden_operations: vec!["shell".to_string()],
            matching_anti_pattern: Some("Fillet radius too large".to_string()),
        }
    }

    #[test]
    fn test_retry_prompt_includes_code_and_error() {
        let error = make_test_error();
        let strategy = make_test_strategy();
        let prompt = build_retry_prompt(
            "import cq\nresult = base.fillet(8.0)",
            "OCP.StdFail_NotDone: BRep_API: not done",
            &error,
            &strategy,
            None,
        );
        assert!(prompt.contains("base.fillet(8.0)"));
        assert!(prompt.contains("BRep_API: not done"));
        assert!(prompt.contains("```python"));
    }

    #[test]
    fn test_retry_prompt_includes_strategy_instruction() {
        let error = make_test_error();
        let strategy = make_test_strategy();
        let prompt = build_retry_prompt("code", "error", &error, &strategy, None);
        assert!(prompt.contains("Reduce ALL fillet radii by 50%"));
    }

    #[test]
    fn test_retry_prompt_includes_anti_pattern() {
        let error = make_test_error();
        let strategy = make_test_strategy();
        let anti_pattern = AntiPatternEntry {
            title: "Fillet radius too large".to_string(),
            wrong_code: "result.fillet(10)".to_string(),
            error_message: "BRep_API: not done".to_string(),
            explanation: "The radius exceeds the smallest edge.".to_string(),
            correct_code: "result.fillet(1.0)".to_string(),
        };
        let prompt =
            build_retry_prompt("code", "error", &error, &strategy, Some(&anti_pattern));
        assert!(prompt.contains("Known anti-pattern"));
        assert!(prompt.contains("Fillet radius too large"));
        assert!(prompt.contains("radius exceeds the smallest edge"));
        assert!(prompt.contains("result.fillet(1.0)"));
    }

    #[test]
    fn test_retry_prompt_without_anti_pattern() {
        let error = make_test_error();
        let strategy = RetryStrategy {
            fix_instruction: "Fix it.".to_string(),
            forbidden_operations: vec![],
            matching_anti_pattern: None,
        };
        let prompt = build_retry_prompt("code", "error", &error, &strategy, None);
        assert!(!prompt.contains("Known anti-pattern"));
        assert!(!prompt.contains("Do NOT use these operations"));
    }

    #[test]
    fn test_retry_prompt_includes_forbidden_operations() {
        let error = make_test_error();
        let strategy = make_test_strategy();
        let prompt = build_retry_prompt("code", "error", &error, &strategy, None);
        assert!(prompt.contains("Do NOT use these operations: shell"));
    }

    #[test]
    fn test_retry_prompt_includes_source_line() {
        let error = make_test_error();
        let strategy = make_test_strategy();
        let prompt = build_retry_prompt("code", "error", &error, &strategy, None);
        assert!(prompt.contains("The failing line: `result = base.fillet(8.0)`"));
    }

    #[test]
    fn test_retry_prompt_includes_failing_operation() {
        let error = make_test_error();
        let strategy = make_test_strategy();
        let prompt = build_retry_prompt("code", "error", &error, &strategy, None);
        assert!(prompt.contains("The failing operation: `fillet`"));
    }
}
