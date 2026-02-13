use serde::Serialize;

use crate::config::AppConfig;

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderInfo {
    pub id: String,
    pub display_name: String,
    pub requires_api_key: bool,
    pub base_url: Option<String>,
    pub models: Vec<ModelInfo>,
    pub allows_custom_model: bool,
}

pub fn get_provider_registry() -> Vec<ProviderInfo> {
    vec![
        ProviderInfo {
            id: "claude".to_string(),
            display_name: "Claude".to_string(),
            requires_api_key: true,
            base_url: None,
            models: vec![
                ModelInfo {
                    id: "claude-sonnet-4-5-20250929".to_string(),
                    display_name: "Sonnet 4.5".to_string(),
                },
                ModelInfo {
                    id: "claude-opus-4-6".to_string(),
                    display_name: "Opus 4.6".to_string(),
                },
            ],
            allows_custom_model: false,
        },
        ProviderInfo {
            id: "openai".to_string(),
            display_name: "OpenAI".to_string(),
            requires_api_key: true,
            base_url: None,
            models: vec![
                ModelInfo {
                    id: "gpt-5.2".to_string(),
                    display_name: "GPT-5.2".to_string(),
                },
                ModelInfo {
                    id: "o3-mini".to_string(),
                    display_name: "o3-mini".to_string(),
                },
            ],
            allows_custom_model: false,
        },
        ProviderInfo {
            id: "deepseek".to_string(),
            display_name: "DeepSeek".to_string(),
            requires_api_key: true,
            base_url: Some("https://api.deepseek.com/v1".to_string()),
            models: vec![
                ModelInfo {
                    id: "deepseek-reasoner".to_string(),
                    display_name: "Reasoner R1".to_string(),
                },
            ],
            allows_custom_model: false,
        },
        ProviderInfo {
            id: "qwen".to_string(),
            display_name: "Qwen".to_string(),
            requires_api_key: true,
            base_url: Some("https://dashscope-intl.aliyuncs.com/compatible-mode/v1".to_string()),
            models: vec![
                ModelInfo {
                    id: "qwen3-coder-plus".to_string(),
                    display_name: "Qwen3 Coder Plus".to_string(),
                },
                ModelInfo {
                    id: "qwen3-max".to_string(),
                    display_name: "Qwen3 Max".to_string(),
                },
            ],
            allows_custom_model: false,
        },
        ProviderInfo {
            id: "kimi".to_string(),
            display_name: "Kimi".to_string(),
            requires_api_key: true,
            base_url: Some("https://api.moonshot.ai/v1".to_string()),
            models: vec![ModelInfo {
                id: "kimi-k2.5".to_string(),
                display_name: "K2.5".to_string(),
            }],
            allows_custom_model: false,
        },
        ProviderInfo {
            id: "gemini".to_string(),
            display_name: "Gemini".to_string(),
            requires_api_key: true,
            base_url: None,
            models: vec![
                ModelInfo {
                    id: "gemini-2.5-pro".to_string(),
                    display_name: "2.5 Pro".to_string(),
                },
                ModelInfo {
                    id: "gemini-2.5-flash".to_string(),
                    display_name: "2.5 Flash".to_string(),
                },
            ],
            allows_custom_model: false,
        },
        ProviderInfo {
            id: "ollama".to_string(),
            display_name: "Ollama".to_string(),
            requires_api_key: false,
            base_url: Some("http://localhost:11434".to_string()),
            models: vec![],
            allows_custom_model: true,
        },
    ]
}

pub fn normalize_config_model(config: &mut AppConfig) -> bool {
    const BANNED_MODELS: [&str; 3] = ["deepseek-chat", "gemini-2.0-flash", "gpt-4.1"];
    let registry = get_provider_registry();
    let provider = registry.iter().find(|p| p.id == config.ai_provider);
    let mut changed = false;

    if BANNED_MODELS.contains(&config.model.as_str()) {
        if let Some(p) = provider {
            if let Some(first) = p.models.first() {
                if config.model != first.id {
                    config.model = first.id.clone();
                    changed = true;
                }
            }
        }
        return changed;
    }

    if let Some(p) = provider {
        if p.allows_custom_model {
            return changed;
        }
        let valid = p.models.iter().any(|m| m.id == config.model);
        if !valid {
            if let Some(first) = p.models.first() {
                config.model = first.id.clone();
                changed = true;
            }
        }
    }

    changed
}

#[cfg(test)]
mod tests {
    use super::normalize_config_model;
    use crate::config::AppConfig;

    #[test]
    fn normalize_banned_model() {
        let mut cfg = AppConfig::default();
        cfg.ai_provider = "deepseek".to_string();
        cfg.model = "deepseek-chat".to_string();
        let changed = normalize_config_model(&mut cfg);
        assert!(changed);
        assert_eq!(cfg.model, "deepseek-reasoner");
    }

    #[test]
    fn normalize_invalid_model_for_provider() {
        let mut cfg = AppConfig::default();
        cfg.ai_provider = "gemini".to_string();
        cfg.model = "unknown-model".to_string();
        let changed = normalize_config_model(&mut cfg);
        assert!(changed);
        assert_eq!(cfg.model, "gemini-2.5-pro");
    }
}
