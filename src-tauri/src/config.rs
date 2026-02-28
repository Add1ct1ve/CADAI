use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GenerationReliabilityProfile {
    ReliabilityFirst,
    Balanced,
    FidelityFirst,
}

impl Default for GenerationReliabilityProfile {
    fn default() -> Self {
        Self::ReliabilityFirst
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewerMode {
    AdvisoryOnly,
    RewriteAllowed,
}

impl Default for ReviewerMode {
    fn default() -> Self {
        Self::AdvisoryOnly
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SemanticBboxMode {
    SemanticAware,
    Legacy,
}

impl Default for SemanticBboxMode {
    fn default() -> Self {
        Self::SemanticAware
    }
}

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
    pub runpod_base_url: Option<String>,
    #[serde(default)]
    pub agent_rules_preset: Option<String>,
    #[serde(default = "default_true")]
    pub enable_code_review: bool,
    #[serde(default = "default_units")]
    pub display_units: String,
    #[serde(default = "default_grid_size")]
    pub grid_size: f64,
    #[serde(default = "default_grid_spacing")]
    pub grid_spacing: f64,
    #[serde(default = "default_snap_translate")]
    pub snap_translate: Option<f64>,
    #[serde(default = "default_snap_rotation")]
    pub snap_rotation: Option<f64>,
    #[serde(default = "default_snap_sketch")]
    pub snap_sketch: Option<f64>,
    #[serde(default)]
    pub enable_consensus: bool,
    #[serde(default)]
    pub auto_approve_plan: bool,
    #[serde(default = "default_true")]
    pub retrieval_enabled: bool,
    #[serde(default = "default_retrieval_token_budget")]
    pub retrieval_token_budget: u32,
    #[serde(default = "default_true")]
    pub telemetry_enabled: bool,
    #[serde(default = "default_max_validation_attempts")]
    pub max_validation_attempts: u32,
    #[serde(default)]
    pub generation_reliability_profile: GenerationReliabilityProfile,
    #[serde(default = "default_true")]
    pub preview_on_partial_failure: bool,
    #[serde(default = "default_max_generation_runtime_seconds")]
    pub max_generation_runtime_seconds: u32,
    #[serde(default = "default_true")]
    pub semantic_contract_strict: bool,
    #[serde(default)]
    pub reviewer_mode: ReviewerMode,
    #[serde(default = "default_true")]
    pub quality_gates_strict: bool,
    #[serde(default = "default_true")]
    pub allow_euler_override: bool,
    #[serde(default)]
    pub semantic_bbox_mode: SemanticBboxMode,
    #[serde(default = "default_true")]
    pub mechanisms_enabled: bool,
    #[serde(default)]
    pub mechanism_import_enabled: bool,
    #[serde(default = "default_mechanism_cache_max_mb")]
    pub mechanism_cache_max_mb: u32,
    #[serde(default = "default_allowed_spdx_licenses")]
    pub allowed_spdx_licenses: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_units() -> String {
    "mm".to_string()
}

fn default_grid_size() -> f64 {
    500.0
}

fn default_grid_spacing() -> f64 {
    2.0
}

fn default_snap_translate() -> Option<f64> {
    Some(1.0)
}

fn default_snap_rotation() -> Option<f64> {
    Some(15.0)
}

fn default_snap_sketch() -> Option<f64> {
    Some(0.5)
}

fn default_retrieval_token_budget() -> u32 {
    3500
}

fn default_max_validation_attempts() -> u32 {
    4
}

fn default_max_generation_runtime_seconds() -> u32 {
    600
}

fn default_mechanism_cache_max_mb() -> u32 {
    512
}

fn default_allowed_spdx_licenses() -> Vec<String> {
    vec![
        "MIT".to_string(),
        "Apache-2.0".to_string(),
        "BSD-2-Clause".to_string(),
        "BSD-3-Clause".to_string(),
        "CC0-1.0".to_string(),
    ]
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ai_provider: "claude".to_string(),
            api_key: None,
            model: "claude-sonnet-4-5-20250929".to_string(),
            python_path: None,
            theme: "dark".to_string(),
            ollama_base_url: None,
            openai_base_url: None,
            runpod_base_url: None,
            agent_rules_preset: None,
            enable_code_review: true,
            display_units: "mm".to_string(),
            grid_size: 500.0,
            grid_spacing: 2.0,
            snap_translate: Some(1.0),
            snap_rotation: Some(15.0),
            snap_sketch: Some(0.5),
            enable_consensus: false,
            auto_approve_plan: false,
            retrieval_enabled: true,
            retrieval_token_budget: default_retrieval_token_budget(),
            telemetry_enabled: true,
            max_validation_attempts: default_max_validation_attempts(),
            generation_reliability_profile: GenerationReliabilityProfile::default(),
            preview_on_partial_failure: true,
            max_generation_runtime_seconds: default_max_generation_runtime_seconds(),
            semantic_contract_strict: true,
            reviewer_mode: ReviewerMode::default(),
            quality_gates_strict: true,
            allow_euler_override: true,
            semantic_bbox_mode: SemanticBboxMode::default(),
            mechanisms_enabled: true,
            mechanism_import_enabled: false,
            mechanism_cache_max_mb: default_mechanism_cache_max_mb(),
            allowed_spdx_licenses: default_allowed_spdx_licenses(),
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
