use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use crate::error::AppError;

/// Top-level agent rules loaded from a YAML configuration file.
/// This structure matches the actual default.yaml schema.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AgentRules {
    pub version: Option<u32>,
    pub coordinate_system: Option<CoordinateSystem>,
    pub spatial_rules: Option<HashMap<String, Vec<String>>>,
    pub code_requirements: Option<CodeRequirements>,
    pub validation: Option<ValidationRules>,
    pub on_error: Option<HashMap<String, Vec<String>>>,
    pub code_style: Option<CodeStyle>,
    pub manufacturing: Option<serde_yaml::Value>,
    pub response_format: Option<HashMap<String, Vec<String>>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoordinateSystem {
    pub description: Option<String>,
    pub x: Option<AxisInfo>,
    pub y: Option<AxisInfo>,
    pub z: Option<AxisInfo>,
    pub origin: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AxisInfo {
    pub direction: Option<String>,
    pub positive: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodeRequirements {
    pub mandatory: Option<Vec<String>>,
    pub forbidden: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ValidationRules {
    pub pre_generation: Option<Vec<serde_yaml::Value>>,
    pub post_generation: Option<Vec<serde_yaml::Value>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodeStyle {
    pub naming: Option<Vec<String>>,
    pub comments: Option<Vec<String>>,
    pub organization: Option<Vec<String>>,
    pub example: Option<String>,
}

impl AgentRules {
    /// Load agent rules from a YAML file.
    #[allow(dead_code)]
    pub fn load_from_file(path: &Path) -> Result<Self, AppError> {
        let contents = std::fs::read_to_string(path)?;
        let rules: AgentRules =
            serde_yaml::from_str(&contents).map_err(|e| AppError::ConfigError(e.to_string()))?;
        Ok(rules)
    }

    /// Create a default (empty) set of rules.
    pub fn default_empty() -> Self {
        Self {
            version: Some(1),
            coordinate_system: None,
            spatial_rules: None,
            code_requirements: None,
            validation: None,
            on_error: None,
            code_style: None,
            manufacturing: None,
            response_format: None,
        }
    }
}
