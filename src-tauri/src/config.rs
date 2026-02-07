use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub ai_provider: String,
    pub api_key: Option<String>,
    pub model: String,
    pub python_path: Option<String>,
    pub theme: String,
    #[serde(default)]
    pub ollama_base_url: Option<String>,
    #[serde(default)]
    pub openai_base_url: Option<String>,
    #[serde(default)]
    pub agent_rules_preset: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ai_provider: "claude".to_string(),
            api_key: None,
            model: "claude-sonnet-4-20250514".to_string(),
            python_path: None,
            theme: "dark".to_string(),
            ollama_base_url: None,
            openai_base_url: None,
            agent_rules_preset: None,
        }
    }
}

impl AppConfig {
    /// Get the path to the config file in app data dir
    pub fn config_path() -> Result<PathBuf, AppError> {
        let data_dir = dirs::config_dir()
            .ok_or_else(|| AppError::ConfigError("Cannot find config directory".into()))?;
        Ok(data_dir.join("cadai-studio").join("config.json"))
    }

    /// Load config from disk, or return default if not found
    pub fn load() -> Result<Self, AppError> {
        let path = Self::config_path()?;
        if path.exists() {
            let contents = std::fs::read_to_string(&path)?;
            let config: AppConfig = serde_json::from_str(&contents)
                .map_err(|e| AppError::ConfigError(e.to_string()))?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// Save config to disk
    pub fn save(&self) -> Result<(), AppError> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;
        Ok(())
    }
}
