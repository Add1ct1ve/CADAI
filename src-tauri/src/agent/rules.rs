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
    pub capabilities: Option<HashMap<String, Vec<String>>>,
    pub advanced_techniques: Option<HashMap<String, Vec<String>>>,
    pub design_thinking: Option<HashMap<String, Vec<String>>>,
    pub response_format: Option<HashMap<String, Vec<String>>>,
    pub cookbook: Option<Vec<CookbookEntry>>,
    pub anti_patterns: Option<Vec<AntiPatternEntry>>,
    pub api_reference: Option<Vec<ApiReferenceEntry>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CookbookEntry {
    pub title: String,
    pub description: Option<String>,
    pub code: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AntiPatternEntry {
    pub title: String,
    pub wrong_code: String,
    pub error_message: String,
    pub explanation: String,
    pub correct_code: String,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ApiReferenceEntry {
    pub operation: String,
    pub signature: String,
    pub returns: String,
    pub params: Vec<String>,
    pub gotchas: Vec<String>,
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

pub(crate) const DEFAULT_YAML: &str = include_str!("../../../agent-rules/default.yaml");
pub(crate) const PRINTING_YAML: &str = include_str!("../../../agent-rules/printing-focused.yaml");
pub(crate) const CNC_YAML: &str = include_str!("../../../agent-rules/cnc-focused.yaml");

impl AgentRules {
    /// Load agent rules from a YAML file.
    #[allow(dead_code)]
    pub fn load_from_file(path: &Path) -> Result<Self, AppError> {
        let contents = std::fs::read_to_string(path)?;
        let rules: AgentRules =
            serde_yaml::from_str(&contents).map_err(|e| AppError::ConfigError(e.to_string()))?;
        Ok(rules)
    }

    /// Load agent rules from an embedded preset by name.
    /// Valid names: "3d-printing", "cnc". Anything else (including None) loads the default preset.
    pub fn from_preset(name: Option<&str>) -> Result<Self, AppError> {
        let yaml_str = match name {
            Some("3d-printing") => PRINTING_YAML,
            Some("cnc") => CNC_YAML,
            _ => DEFAULT_YAML,
        };
        serde_yaml::from_str(yaml_str)
            .map_err(|e| AppError::ConfigError(format!("Failed to parse agent rules: {}", e)))
    }

    /// Create a default (empty) set of rules.
    #[allow(dead_code)]
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
            capabilities: None,
            advanced_techniques: None,
            design_thinking: None,
            response_format: None,
            cookbook: None,
            anti_patterns: None,
            api_reference: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── YAML Deserialization ────────────────────────────────────────────

    #[test]
    fn test_default_yaml_parses() {
        let rules: AgentRules = serde_yaml::from_str(DEFAULT_YAML)
            .expect("default.yaml should parse without errors");
        assert_eq!(rules.version, Some(1));
    }

    #[test]
    fn test_printing_yaml_parses() {
        let rules: AgentRules = serde_yaml::from_str(PRINTING_YAML)
            .expect("printing-focused.yaml should parse without errors");
        assert_eq!(rules.version, Some(1));
    }

    #[test]
    fn test_cnc_yaml_parses() {
        let rules: AgentRules = serde_yaml::from_str(CNC_YAML)
            .expect("cnc-focused.yaml should parse without errors");
        assert_eq!(rules.version, Some(1));
    }

    // ── from_preset() ──────────────────────────────────────────────────

    #[test]
    fn test_from_preset_default() {
        let rules = AgentRules::from_preset(None).expect("default preset should load");
        assert!(rules.capabilities.is_some());
        assert!(rules.advanced_techniques.is_some());
    }

    #[test]
    fn test_from_preset_printing() {
        let rules =
            AgentRules::from_preset(Some("3d-printing")).expect("printing preset should load");
        assert!(rules.capabilities.is_some());
        assert!(rules.advanced_techniques.is_some());
    }

    #[test]
    fn test_from_preset_cnc() {
        let rules = AgentRules::from_preset(Some("cnc")).expect("cnc preset should load");
        assert!(rules.capabilities.is_some());
        assert!(rules.advanced_techniques.is_some());
    }

    #[test]
    fn test_from_preset_unknown_falls_back_to_default() {
        let rules = AgentRules::from_preset(Some("laser"))
            .expect("unknown preset should fall back to default");
        assert_eq!(rules.version, Some(1));
        assert!(rules.capabilities.is_some());
    }

    // ── Capabilities section ───────────────────────────────────────────

    #[test]
    fn test_default_capabilities_has_all_categories() {
        let rules = AgentRules::from_preset(None).unwrap();
        let caps = rules.capabilities.as_ref().unwrap();
        assert!(caps.contains_key("excels_at"), "missing excels_at");
        assert!(caps.contains_key("limitations"), "missing limitations");
        assert!(
            caps.contains_key("strategy_for_complex_requests"),
            "missing strategy"
        );
    }

    #[test]
    fn test_capabilities_limitations_mention_organic() {
        let rules = AgentRules::from_preset(None).unwrap();
        let caps = rules.capabilities.as_ref().unwrap();
        let limitations = caps.get("limitations").unwrap();
        assert!(
            limitations.iter().any(|l| l.contains("organic")),
            "limitations should mention organic surfaces"
        );
    }

    // ── Advanced Techniques section ────────────────────────────────────

    #[test]
    fn test_default_advanced_techniques_has_all_categories() {
        let rules = AgentRules::from_preset(None).unwrap();
        let tech = rules.advanced_techniques.as_ref().unwrap();
        assert!(
            tech.contains_key("profile_based_modeling"),
            "missing profile_based_modeling"
        );
        assert!(
            tech.contains_key("approximating_organic_shapes"),
            "missing approximating_organic_shapes"
        );
        assert!(
            tech.contains_key("common_pitfalls"),
            "missing common_pitfalls"
        );
    }

    #[test]
    fn test_common_pitfalls_mention_fillet_radius() {
        let rules = AgentRules::from_preset(None).unwrap();
        let tech = rules.advanced_techniques.as_ref().unwrap();
        let pitfalls = tech.get("common_pitfalls").unwrap();
        assert!(
            pitfalls.iter().any(|p| p.contains("Fillet radius")),
            "pitfalls should mention fillet radius"
        );
    }

    // ── Cookbook ────────────────────────────────────────────────────────

    #[test]
    fn test_default_cookbook_has_48_recipes() {
        let rules = AgentRules::from_preset(None).unwrap();
        let cookbook = rules.cookbook.as_ref().unwrap();
        assert_eq!(cookbook.len(), 48, "cookbook should have 48 recipes");
    }

    #[test]
    fn test_printing_cookbook_has_48_recipes() {
        let rules = AgentRules::from_preset(Some("3d-printing")).unwrap();
        let cookbook = rules.cookbook.as_ref().unwrap();
        assert_eq!(cookbook.len(), 48, "printing cookbook should have 48 recipes");
    }

    #[test]
    fn test_cnc_cookbook_has_48_recipes() {
        let rules = AgentRules::from_preset(Some("cnc")).unwrap();
        let cookbook = rules.cookbook.as_ref().unwrap();
        assert_eq!(cookbook.len(), 48, "cnc cookbook should have 48 recipes");
    }

    #[test]
    fn test_new_cookbook_recipes_present() {
        let rules = AgentRules::from_preset(None).unwrap();
        let cookbook = rules.cookbook.as_ref().unwrap();
        let titles: Vec<&str> = cookbook.iter().map(|e| e.title.as_str()).collect();
        // Original recipes
        assert!(titles.iter().any(|t| t.contains("Revolve")));
        assert!(titles.iter().any(|t| t.contains("Sweep")));
        assert!(titles.iter().any(|t| t.contains("Loft")));
        assert!(titles.iter().any(|t| t.contains("Spline")));
        assert!(titles.iter().any(|t| t.contains("Text")));
        assert!(titles.iter().any(|t| t.contains("Circular pattern")));
        assert!(titles.iter().any(|t| t.contains("helmet")));
        assert!(titles.iter().any(|t| t.contains("Countersink")));
        assert!(titles.iter().any(|t| t.contains("Multi-body")));
        // Phase 1.1 expanded recipes
        assert!(titles.iter().any(|t| t.contains("Pipe elbow")));
        assert!(titles.iter().any(|t| t.contains("T-junction")));
        assert!(titles.iter().any(|t| t.contains("Hex bolt")));
        assert!(titles.iter().any(|t| t.contains("spring")));
        assert!(titles.iter().any(|t| t.contains("Bearing seat")));
        assert!(titles.iter().any(|t| t.contains("Snap-fit")));
        assert!(titles.iter().any(|t| t.contains("Dovetail")));
        assert!(titles.iter().any(|t| t.contains("L-bracket")));
        assert!(titles.iter().any(|t| t.contains("Spur gear")));
        assert!(titles.iter().any(|t| t.contains("Pulley")));
        assert!(titles.iter().any(|t| t.contains("hinge")));
        assert!(titles.iter().any(|t| t.contains("Knurled")));
        assert!(titles.iter().any(|t| t.contains("USB-C")));
        assert!(titles.iter().any(|t| t.contains("Raspberry Pi")));
        assert!(titles.iter().any(|t| t.contains("Keychain")));
    }

    #[test]
    fn test_all_cookbook_recipes_have_import_and_result() {
        for preset in &[None, Some("3d-printing"), Some("cnc")] {
            let rules = AgentRules::from_preset(*preset).unwrap();
            let cookbook = rules.cookbook.as_ref().unwrap();
            for entry in cookbook {
                assert!(
                    entry.code.contains("import cadquery"),
                    "Recipe '{}' in preset {:?} missing 'import cadquery'",
                    entry.title,
                    preset
                );
                assert!(
                    entry.code.contains("result"),
                    "Recipe '{}' in preset {:?} missing 'result' variable",
                    entry.title,
                    preset
                );
            }
        }
    }

    // ── Validation & Manufacturing ─────────────────────────────────────

    #[test]
    fn test_all_presets_have_validation() {
        for preset in &[None, Some("3d-printing"), Some("cnc")] {
            let rules = AgentRules::from_preset(*preset).unwrap();
            assert!(
                rules.validation.is_some(),
                "preset {:?} should have validation",
                preset
            );
            let v = rules.validation.as_ref().unwrap();
            assert!(v.pre_generation.is_some());
            assert!(v.post_generation.is_some());
        }
    }

    #[test]
    fn test_all_presets_have_manufacturing() {
        for preset in &[None, Some("3d-printing"), Some("cnc")] {
            let rules = AgentRules::from_preset(*preset).unwrap();
            assert!(
                rules.manufacturing.is_some(),
                "preset {:?} should have manufacturing",
                preset
            );
        }
    }

    // ── Anti-Patterns ─────────────────────────────────────────────────

    #[test]
    fn test_default_anti_patterns_has_10_entries() {
        let rules = AgentRules::from_preset(None).unwrap();
        let ap = rules.anti_patterns.as_ref().unwrap();
        assert_eq!(ap.len(), 10, "default should have 10 anti-patterns");
    }

    #[test]
    fn test_printing_anti_patterns_has_10_entries() {
        let rules = AgentRules::from_preset(Some("3d-printing")).unwrap();
        let ap = rules.anti_patterns.as_ref().unwrap();
        assert_eq!(ap.len(), 10, "printing should have 10 anti-patterns");
    }

    #[test]
    fn test_cnc_anti_patterns_has_10_entries() {
        let rules = AgentRules::from_preset(Some("cnc")).unwrap();
        let ap = rules.anti_patterns.as_ref().unwrap();
        assert_eq!(ap.len(), 10, "cnc should have 10 anti-patterns");
    }

    #[test]
    fn test_anti_pattern_titles_present() {
        let rules = AgentRules::from_preset(None).unwrap();
        let ap = rules.anti_patterns.as_ref().unwrap();
        let titles: Vec<&str> = ap.iter().map(|e| e.title.as_str()).collect();
        assert!(titles.iter().any(|t| t.contains("Fillet before boolean")));
        assert!(titles.iter().any(|t| t.contains("Shell on complex")));
        assert!(titles.iter().any(|t| t.contains("Revolve profile")));
        assert!(titles.iter().any(|t| t.contains("translate()")));
        assert!(titles.iter().any(|t| t.contains("Sweep path")));
        assert!(titles.iter().any(|t| t.contains("wrong face")));
    }

    #[test]
    fn test_all_anti_patterns_have_required_fields() {
        for preset in &[None, Some("3d-printing"), Some("cnc")] {
            let rules = AgentRules::from_preset(*preset).unwrap();
            let ap = rules.anti_patterns.as_ref().unwrap();
            for entry in ap {
                assert!(
                    !entry.title.is_empty(),
                    "Anti-pattern in preset {:?} has empty title",
                    preset
                );
                assert!(
                    !entry.wrong_code.is_empty(),
                    "Anti-pattern '{}' in preset {:?} has empty wrong_code",
                    entry.title,
                    preset
                );
                assert!(
                    !entry.error_message.is_empty(),
                    "Anti-pattern '{}' in preset {:?} has empty error_message",
                    entry.title,
                    preset
                );
                assert!(
                    !entry.explanation.is_empty(),
                    "Anti-pattern '{}' in preset {:?} has empty explanation",
                    entry.title,
                    preset
                );
                assert!(
                    !entry.correct_code.is_empty(),
                    "Anti-pattern '{}' in preset {:?} has empty correct_code",
                    entry.title,
                    preset
                );
            }
        }
    }

    // ── API Reference ────────────────────────────────────────────────

    #[test]
    fn test_default_api_reference_has_8_entries() {
        let rules = AgentRules::from_preset(None).unwrap();
        let api = rules.api_reference.as_ref().unwrap();
        assert_eq!(api.len(), 8, "default should have 8 API reference entries");
    }

    #[test]
    fn test_printing_api_reference_has_8_entries() {
        let rules = AgentRules::from_preset(Some("3d-printing")).unwrap();
        let api = rules.api_reference.as_ref().unwrap();
        assert_eq!(api.len(), 8, "printing should have 8 API reference entries");
    }

    #[test]
    fn test_cnc_api_reference_has_8_entries() {
        let rules = AgentRules::from_preset(Some("cnc")).unwrap();
        let api = rules.api_reference.as_ref().unwrap();
        assert_eq!(api.len(), 8, "cnc should have 8 API reference entries");
    }

    #[test]
    fn test_api_reference_operations_present() {
        let rules = AgentRules::from_preset(None).unwrap();
        let api = rules.api_reference.as_ref().unwrap();
        let ops: Vec<&str> = api.iter().map(|e| e.operation.as_str()).collect();
        assert!(ops.iter().any(|o| o.contains("loft")));
        assert!(ops.iter().any(|o| o.contains("sweep")));
        assert!(ops.iter().any(|o| o.contains("revolve")));
        assert!(ops.iter().any(|o| o.contains("shell")));
        assert!(ops.iter().any(|o| o.contains("Selector")));
        assert!(ops.iter().any(|o| o.contains("Workplane")));
        assert!(ops.iter().any(|o| o.contains("pushPoints")));
        assert!(ops.iter().any(|o| o.contains("tag")));
    }

    #[test]
    fn test_all_api_reference_entries_have_required_fields() {
        for preset in &[None, Some("3d-printing"), Some("cnc")] {
            let rules = AgentRules::from_preset(*preset).unwrap();
            let api = rules.api_reference.as_ref().unwrap();
            for entry in api {
                assert!(
                    !entry.operation.is_empty(),
                    "API ref in preset {:?} has empty operation",
                    preset
                );
                assert!(
                    !entry.signature.is_empty(),
                    "API ref '{}' in preset {:?} has empty signature",
                    entry.operation,
                    preset
                );
                assert!(
                    !entry.returns.is_empty(),
                    "API ref '{}' in preset {:?} has empty returns",
                    entry.operation,
                    preset
                );
                assert!(
                    !entry.params.is_empty(),
                    "API ref '{}' in preset {:?} has empty params",
                    entry.operation,
                    preset
                );
                assert!(
                    !entry.gotchas.is_empty(),
                    "API ref '{}' in preset {:?} has empty gotchas",
                    entry.operation,
                    preset
                );
            }
        }
    }

    // ── default_empty() ────────────────────────────────────────────────

    #[test]
    fn test_default_empty_has_none_for_new_fields() {
        let rules = AgentRules::default_empty();
        assert!(rules.capabilities.is_none());
        assert!(rules.advanced_techniques.is_none());
        assert!(rules.design_thinking.is_none());
        assert!(rules.anti_patterns.is_none());
        assert!(rules.api_reference.is_none());
    }

    // ── Print-specific extras ──────────────────────────────────────────

    #[test]
    fn test_printing_has_print_specific_capability() {
        let rules = AgentRules::from_preset(Some("3d-printing")).unwrap();
        let caps = rules.capabilities.as_ref().unwrap();
        let excels = caps.get("excels_at").unwrap();
        assert!(
            excels.iter().any(|e| e.contains("Print-ready")),
            "printing preset should mention print-ready models"
        );
    }

    #[test]
    fn test_default_has_design_thinking() {
        let rules = AgentRules::from_preset(None).unwrap();
        let dt = rules.design_thinking.as_ref().unwrap();
        assert!(
            dt.contains_key("mandatory_before_code"),
            "missing mandatory_before_code"
        );
        assert!(
            dt.contains_key("for_organic_shapes"),
            "missing for_organic_shapes"
        );
        assert!(
            dt.contains_key("for_complex_objects"),
            "missing for_complex_objects"
        );
    }

    #[test]
    fn test_cnc_has_cnc_specific_capability() {
        let rules = AgentRules::from_preset(Some("cnc")).unwrap();
        let caps = rules.capabilities.as_ref().unwrap();
        let excels = caps.get("excels_at").unwrap();
        assert!(
            excels.iter().any(|e| e.contains("CNC-ready")),
            "cnc preset should mention CNC-ready models"
        );
    }
}
