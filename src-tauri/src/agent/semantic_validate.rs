use regex::Regex;
use serde::Serialize;

use crate::agent::executor::PostGeometryValidationReport;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OrientationPolicy {
    BaseNearZ0,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExpectedBboxMm {
    pub sorted_extents_mm: [f64; 3],
    pub tolerance_ratio: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticPartContract {
    pub expected_components: u64,
    pub must_be_editable_single_solid: bool,
    pub expected_bbox_mm: Option<ExpectedBboxMm>,
    pub orientation_policy: OrientationPolicy,
    pub required_feature_hints: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticValidationResult {
    pub passed: bool,
    pub findings: Vec<String>,
}

fn tokenize_words(input: &str) -> Vec<String> {
    let re = Regex::new(r"[A-Za-z0-9_]+").expect("valid token regex");
    re.find_iter(&input.to_lowercase())
        .map(|m| m.as_str().to_string())
        .collect()
}

fn contains_token(tokens: &[String], token: &str) -> bool {
    tokens.iter().any(|t| t == token)
}

fn contains_phrase_tokens(tokens: &[String], first: &str, second: &str) -> bool {
    tokens
        .windows(2)
        .any(|window| window[0] == first && window[1] == second)
}

fn is_back_plate_like_part(part_name: &str) -> bool {
    let tokens = tokenize_words(part_name);
    contains_token(&tokens, "backplate")
        || contains_token(&tokens, "back_plate")
        || contains_phrase_tokens(&tokens, "back", "plate")
        || contains_token(&tokens, "cover")
        || contains_token(&tokens, "lid")
        || contains_token(&tokens, "backplate")
        || contains_token(&tokens, "back") && contains_token(&tokens, "plate")
}

fn parse_named_dimension(description: &str, labels: &[&str]) -> Option<f64> {
    for label in labels {
        let pat = format!(
            r"(?i)\b{}\b[^0-9-]*(-?\d+(?:\.\d+)?)\s*mm",
            regex::escape(label)
        );
        if let Some(val) = Regex::new(&pat)
            .ok()
            .and_then(|re| re.captures(description))
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<f64>().ok())
            .filter(|v| *v > 0.0)
        {
            return Some(val);
        }
    }
    None
}

pub fn infer_envelope_dimensions_mm(description: &str) -> Option<[f64; 3]> {
    let compact = Regex::new(
        r"(?i)(\d+(?:\.\d+)?)\s*[x×]\s*(\d+(?:\.\d+)?)\s*[x×]\s*(\d+(?:\.\d+)?)\s*(?:mm)?",
    )
    .unwrap();
    if let Some(c) = compact.captures(description) {
        let mut vals = [0.0_f64; 3];
        for (i, slot) in vals.iter_mut().enumerate() {
            *slot = c
                .get(i + 1)
                .and_then(|m| m.as_str().parse::<f64>().ok())
                .unwrap_or(0.0);
        }
        if vals.iter().all(|v| *v > 0.0) {
            return Some(vals);
        }
    }

    // Semantic-aware envelope extraction:
    // - Prefer explicit outer/envelope dimensions.
    // - Do not use feature values like thickness/depth as a fake third axis.
    let length = parse_named_dimension(description, &["overall length", "outer length", "length"]);
    let width = parse_named_dimension(description, &["overall width", "outer width", "width"]);
    let height = parse_named_dimension(
        description,
        &[
            "overall height",
            "outer height",
            "envelope height",
            "height",
        ],
    );

    match (length, width, height) {
        (Some(a), Some(b), Some(c)) => Some([a, b, c]),
        _ => None,
    }
}

fn infer_required_feature_hints(part_name: &str, description: &str) -> Vec<String> {
    let mut hints = Vec::new();
    let name_tokens = tokenize_words(part_name);
    let lower_desc = description.to_lowercase();
    let combined = format!("{} {}", part_name.to_lowercase(), lower_desc);

    // Only require lip/ridge features when the PART NAME indicates it's a
    // back plate, lid, or cover.  Checking the description would cause false
    // positives for housing parts whose descriptions reference "back plate"
    // as a cross-part constraint (e.g. "internal ledge for back plate").
    let is_plate_name = is_back_plate_like_part(part_name)
        || contains_token(&name_tokens, "plate")
        || contains_token(&name_tokens, "cover")
        || contains_token(&name_tokens, "lid");
    if is_plate_name {
        if combined.contains("lip") {
            hints.push("lip".to_string());
        }
        if combined.contains("ridge") || combined.contains("o-ring") || combined.contains("oring") {
            hints.push("ridge".to_string());
        }
    }

    if combined.contains("slot") {
        hints.push("slot".to_string());
    }

    hints.sort();
    hints.dedup();
    hints
}

pub fn build_default_contract(part_name: &str, description: &str) -> SemanticPartContract {
    let expected_bbox_mm = infer_envelope_dimensions_mm(description).map(|mut dims| {
        dims.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        ExpectedBboxMm {
            sorted_extents_mm: dims,
            tolerance_ratio: 0.70,
        }
    });

    SemanticPartContract {
        expected_components: 1,
        must_be_editable_single_solid: true,
        expected_bbox_mm,
        orientation_policy: OrientationPolicy::BaseNearZ0,
        required_feature_hints: infer_required_feature_hints(part_name, description),
    }
}

pub fn validate_part_semantics(
    contract: &SemanticPartContract,
    report: &PostGeometryValidationReport,
    code: &str,
) -> SemanticValidationResult {
    let mut findings = Vec::new();

    if contract.must_be_editable_single_solid
        && report.component_count != contract.expected_components
    {
        findings.push(format!(
            "component count {} violates contract (expected {}).",
            report.component_count, contract.expected_components
        ));
    }

    if let Some(ref expected_bbox) = contract.expected_bbox_mm {
        let mut actual = [
            (report.bounds_max[0] - report.bounds_min[0]).abs(),
            (report.bounds_max[1] - report.bounds_min[1]).abs(),
            (report.bounds_max[2] - report.bounds_min[2]).abs(),
        ];
        actual.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

        for (idx, expected) in expected_bbox.sorted_extents_mm.iter().enumerate() {
            let min_allowed = expected * (1.0 - expected_bbox.tolerance_ratio);
            let max_allowed = expected * (1.0 + expected_bbox.tolerance_ratio);
            let got = actual[idx];
            if got < min_allowed || got > max_allowed {
                findings.push(format!(
                    "bbox extent {} out of contract: got {:.2}mm, expected {:.2}mm ±{}%",
                    idx + 1,
                    got,
                    expected,
                    (expected_bbox.tolerance_ratio * 100.0).round()
                ));
                break;
            }
        }
    }

    if matches!(contract.orientation_policy, OrientationPolicy::BaseNearZ0) {
        let z_min = report.bounds_min[2];
        let z_height = (report.bounds_max[2] - report.bounds_min[2]).abs();
        let allowed = 2.0_f64.max(z_height * 0.25);
        if z_min.abs() > allowed {
            findings.push(format!(
                "orientation policy violated: base expected near Z=0, but z_min is {:.2}mm",
                z_min
            ));
        }
    }

    let code_lower = code.to_lowercase();
    for hint in &contract.required_feature_hints {
        if !code_lower.contains(hint) {
            findings.push(format!(
                "required feature hint '{}' not reflected in generated code.",
                hint
            ));
        }
    }

    SemanticValidationResult {
        passed: findings.is_empty(),
        findings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_report() -> PostGeometryValidationReport {
        PostGeometryValidationReport {
            watertight: true,
            manifold: true,
            degenerate_faces: 0,
            euler_number: 2,
            triangle_count: 100,
            component_count: 1,
            bounds_min: [0.0, 0.0, 0.0],
            bounds_max: [40.0, 20.0, 10.0],
            volume: 1000.0,
            bbox_ok: true,
            warnings: vec![],
        }
    }

    #[test]
    fn rejects_split_part_component_count() {
        let mut report = base_report();
        report.component_count = 2;
        let contract = build_default_contract("back_plate", "size 40x20x10mm with lip and ridge");
        let result = validate_part_semantics(&contract, &report, "result = cq.Workplane('XY')");
        assert!(!result.passed);
        assert!(result
            .findings
            .iter()
            .any(|f| f.contains("component count")));
    }

    #[test]
    fn rejects_bbox_out_of_contract() {
        let mut report = base_report();
        report.bounds_max = [200.0, 120.0, 80.0];
        let contract = build_default_contract("housing", "outer dimensions 40x20x10mm");
        let result = validate_part_semantics(&contract, &report, "result = cq.Workplane('XY')");
        assert!(!result.passed);
        assert!(result.findings.iter().any(|f| f.contains("bbox extent")));
    }

    #[test]
    fn rejects_orientation_policy_violation() {
        let mut report = base_report();
        report.bounds_min = [0.0, 0.0, -25.0];
        report.bounds_max = [40.0, 20.0, -5.0];
        let contract = build_default_contract("housing", "40x20x10mm");
        let result = validate_part_semantics(&contract, &report, "result = cq.Workplane('XY')");
        assert!(!result.passed);
        assert!(result
            .findings
            .iter()
            .any(|f| f.contains("orientation policy")));
    }

    #[test]
    fn whoop_housing_contract_accepts_valid_geometry() {
        let contract = build_default_contract(
            "housing",
            "Single editable housing solid. Footprint 42x28mm, top 7.5/5mm, wall 1.8mm.",
        );
        let mut report = base_report();
        report.bounds_min = [-21.0, -14.0, 0.0];
        report.bounds_max = [21.0, 14.0, 7.5];
        report.component_count = 1;
        let code = "result = housing";
        let result = validate_part_semantics(&contract, &report, code);
        assert!(
            result.passed,
            "valid Whoop housing should pass semantic validation, findings: {:?}",
            result.findings
        );
    }

    #[test]
    fn whoop_backplate_contract_accepts_valid_geometry() {
        let contract = build_default_contract(
            "back_plate",
            "Single editable back plate solid. Base 30x24mm, thickness 1.5mm.",
        );
        let mut report = base_report();
        report.bounds_min = [-15.0, -12.0, 0.0];
        report.bounds_max = [15.0, 12.0, 1.5];
        report.component_count = 1;
        let code = "result = back_plate";
        let result = validate_part_semantics(&contract, &report, code);
        assert!(
            result.passed,
            "valid Whoop back_plate should pass semantic validation, findings: {:?}",
            result.findings
        );
    }

    #[test]
    fn whoop_housing_rejects_two_component_solid() {
        let contract = build_default_contract(
            "housing",
            "Single editable housing solid. Footprint 42x28mm.",
        );
        let mut report = base_report();
        report.bounds_min = [-21.0, -14.0, 0.0];
        report.bounds_max = [21.0, 14.0, 7.5];
        report.component_count = 2; // Two disconnected solids — invalid
        let result = validate_part_semantics(&contract, &report, "result = housing");
        assert!(
            !result.passed,
            "housing with 2 components should fail (must be single editable solid)"
        );
    }

    #[test]
    fn does_not_false_match_template_as_plate() {
        let hints = infer_required_feature_hints(
            "template_part",
            "generic generated part with slots and chamfer",
        );
        assert!(
            !hints.contains(&"lip".to_string()) && !hints.contains(&"ridge".to_string()),
            "template_part must not be treated as back plate"
        );
    }

    #[test]
    fn does_not_false_match_solid_as_lid() {
        let hints = infer_required_feature_hints("housing", "solid side wall, no cutout");
        assert!(
            !hints.contains(&"lip".to_string()) && !hints.contains(&"ridge".to_string()),
            "word 'solid' must not trigger lid/cover plate hints"
        );
    }

    #[test]
    fn infer_envelope_dimensions_ignores_feature_only_values() {
        let dims = infer_envelope_dimensions_mm(
            "Outer dimensions length 42mm width 28mm. Wall thickness 1.8mm. Top thickness 1.5mm. Height 7.5mm.",
        )
        .expect("should infer envelope dimensions");
        assert_eq!(dims, [42.0, 28.0, 7.5]);
    }
}
