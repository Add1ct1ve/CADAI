use serde::Serialize;

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
                    id: "deepseek-chat".to_string(),
                    display_name: "Chat V3.2".to_string(),
                },
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
            id: "runpod".to_string(),
            display_name: "RunPod (Caiden2)".to_string(),
            requires_api_key: true,
            base_url: Some(
                "https://api.runpod.ai/v2/YOUR_ENDPOINT_ID/openai/v1".to_string(),
            ),
            models: vec![ModelInfo {
                id: "Add1ct1ve/caiden2-build123d-32b".to_string(),
                display_name: "Caiden2 Build123d 32B".to_string(),
            }],
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
