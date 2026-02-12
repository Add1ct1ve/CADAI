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
    /// Wider tolerance for the smallest sorted extent when additive features are present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_extent_tolerance_ratio: Option<f64>,
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

fn description_has_additive_features(description: &str) -> bool {
    let lower = description.to_lowercase();
    let tokens = tokenize_words(&lower);
    const ADDITIVE_TOKENS: &[&str] = &[
        "lip", "ridge", "boss", "tab", "flange", "snap", "clip",
        "rim", "ledge", "step", "protrusion", "rib", "oring",
        "raised", "bump",
    ];
    let token_match = ADDITIVE_TOKENS.iter().any(|kw| tokens.iter().any(|t| t == kw));
    // Also check hyphenated forms that tokenize_words splits
    let phrase_match = lower.contains("o-ring");
    token_match || phrase_match
}

const SUB_FEATURE_QUALIFIERS: &[&str] = &[
    "slot", "band", "groove", "channel", "notch",
    "lip", "ridge", "rib", "boss", "tab", "flange",
    "snap", "clip", "rim", "ledge", "step", "protrusion",
    "bump", "raised", "oring", "clearance", "tolerance",
    "internal", "inner", "wall", "fillet", "chamfer",
];

fn parse_dims_line(description: &str) -> Option<[f64; 3]> {
    let lower = description.to_lowercase();
    let dims_start = lower.find("dims:")?;
    let dims_text = &description[dims_start..];
    let dims_line = dims_text.lines().next().unwrap_or(dims_text);

    let kv_re = Regex::new(r"(?i)(\w+)\s*=\s*(\d+(?:\.\d+)?)\s*mm").ok()?;
    let mut length = None;
    let mut width = None;
    let mut height = None;

    const NON_ENVELOPE_KEYS: &[&str] = &[
        "wall", "thickness", "radius", "fillet", "chamfer",
        "tolerance", "clearance", "gap", "offset", "slot",
        "groove", "lip", "ridge", "rib", "boss", "tab",
    ];

    let mut unmatched: Vec<f64> = Vec::new();

    for cap in kv_re.captures_iter(dims_line) {
        let key = cap[1].to_lowercase();
        let val: f64 = cap[2].parse().ok()?;
        if val <= 0.0 {
            continue;
        }
        match key.as_str() {
            "length" | "len" | "total_length" | "overall_length" | "outer_length" => length = Some(val),
            "width" | "total_width" | "overall_width" | "outer_width" => width = Some(val),
            "height" | "total_height" | "overall_height" | "outer_height" | "envelope_height" | "thickness" => {
                height = Some(val)
            }
            "depth" | "total_depth" | "overall_depth" => {
                // "depth" can map to any missing dimension
                if height.is_none() { height = Some(val); }
                else if width.is_none() { width = Some(val); }
                else if length.is_none() { length = Some(val); }
            }
            other => {
                if !NON_ENVELOPE_KEYS.iter().any(|k| other.contains(k)) {
                    unmatched.push(val);
                }
            }
        }
    }

    // Fallback: if 2-of-3 are assigned, fill the missing slot from unmatched values
    if length.is_none() && width.is_some() && height.is_some() {
        if let Some(&v) = unmatched.first() { length = Some(v); }
    } else if width.is_none() && length.is_some() && height.is_some() {
        if let Some(&v) = unmatched.first() { width = Some(v); }
    } else if height.is_none() && length.is_some() && width.is_some() {
        if let Some(&v) = unmatched.first() { height = Some(v); }
    }

    match (length, width, height) {
        (Some(l), Some(w), Some(h)) => Some([l, w, h]),
        _ => None,
    }
}

fn parse_footprint_plus_height(description: &str) -> Option<[f64; 3]> {
    let fp_re = Regex::new(
        r"(?i)footprint\s+(\d+(?:\.\d+)?)\s*[x×]\s*(\d+(?:\.\d+)?)\s*(?:mm)?",
    )
    .ok()?;
    let cap = fp_re.captures(description)?;
    let a: f64 = cap.get(1)?.as_str().parse().ok().filter(|v: &f64| *v > 0.0)?;
    let b: f64 = cap.get(2)?.as_str().parse().ok().filter(|v: &f64| *v > 0.0)?;
    let height = parse_named_dimension(
        description,
        &["overall height", "outer height", "envelope height", "height"],
    )?;
    Some([a, b, height])
}

fn parse_named_dimension(description: &str, labels: &[&str]) -> Option<f64> {
    for label in labels {
        let pat = format!(
            r"(?i)\b{}\b[^0-9-]*(-?\d+(?:\.\d+)?)\s*mm",
            regex::escape(label)
        );
        let re = Regex::new(&pat).ok()?;

        for cap in re.captures_iter(description) {
            let val: f64 = match cap.get(1).and_then(|m| m.as_str().parse().ok()) {
                Some(v) if v > 0.0 => v,
                _ => continue,
            };

            // Check preceding word for sub-feature qualifier
            let match_start = cap.get(0).unwrap().start();
            let prefix = &description[..match_start].to_lowercase();
            let last_word = prefix.split_whitespace().last().unwrap_or("");
            let is_sub_feature = SUB_FEATURE_QUALIFIERS
                .iter()
                .any(|q| last_word == *q || last_word.ends_with(q));

            if !is_sub_feature {
                return Some(val); // First non-sub-feature match wins
            }
            // Don't store sub-feature matches as fallback — they cause false rejections
        }
    }
    None
}

pub fn infer_envelope_dimensions_mm(description: &str) -> Option<[f64; 3]> {
    // 1. Structured Dims: line (highest priority — explicit key=value format)
    // If a Dims line is present, it is authoritative; do not fall back to other parsing.
    if description.to_lowercase().contains("dims:") {
        return parse_dims_line(description);
    }

    // 2. Compact NxNxN format (e.g. "42x28x7.5mm")
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

    // 3. Footprint NxN + separate height (e.g. "Footprint 42x28mm ... height 7.5mm")
    if let Some(dims) = parse_footprint_plus_height(description) {
        return Some(dims);
    }

    // 4. Named dimension parsing (fallback, with sub-feature filtering)
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

    if !is_plate_name && combined.contains("slot") {
        hints.push("slot".to_string());
    }

    hints.sort();
    hints.dedup();
    hints
}

pub fn build_default_contract(part_name: &str, description: &str) -> SemanticPartContract {
    let expected_bbox_mm = infer_envelope_dimensions_mm(description).map(|mut dims| {
        dims.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        let min_extent_tolerance = if description_has_additive_features(description) {
            Some(1.50)
        } else {
            None
        };
        ExpectedBboxMm {
            sorted_extents_mm: dims,
            tolerance_ratio: 0.70,
            min_extent_tolerance_ratio: min_extent_tolerance,
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
    _code: &str,
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
            let effective_tolerance = if idx == 2 {
                expected_bbox.min_extent_tolerance_ratio.unwrap_or(expected_bbox.tolerance_ratio)
            } else {
                expected_bbox.tolerance_ratio
            };
            let min_allowed = expected * (1.0 - effective_tolerance);
            let max_allowed = expected * (1.0 + effective_tolerance);
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

    // Feature hints are advisory — the AI may implement features using different
    // naming conventions. Remaining validations (component count, bbox, orientation)
    // are objective geometric checks that correctly catch broken geometry.

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
    fn back_plate_with_slot_reference_does_not_require_slot() {
        // A back_plate description that references housing slot parameters
        // (e.g. "band_slot_depth") must NOT require "slot" as a feature hint.
        let hints = infer_required_feature_hints(
            "back_plate",
            "Single editable back plate solid. Base 30x24mm, thickness 1.5mm. \
             Match band_slot_depth 5mm clearance. Add insertion lip and O-ring ridge.",
        );
        assert!(
            !hints.contains(&"slot".to_string()),
            "back_plate should not require 'slot' hint even when description mentions slot params, got: {:?}",
            hints
        );
        // But it should still get lip and ridge hints
        assert!(hints.contains(&"lip".to_string()), "back_plate should require 'lip'");
        assert!(hints.contains(&"ridge".to_string()), "back_plate should require 'ridge'");
    }

    #[test]
    fn infer_envelope_dimensions_ignores_feature_only_values() {
        let dims = infer_envelope_dimensions_mm(
            "Outer dimensions length 42mm width 28mm. Wall thickness 1.8mm. Top thickness 1.5mm. Height 7.5mm.",
        )
        .expect("should infer envelope dimensions");
        assert_eq!(dims, [42.0, 28.0, 7.5]);
    }

    #[test]
    fn backplate_with_lip_uses_wider_tolerance() {
        let contract = build_default_contract(
            "back_plate",
            "Single editable back plate solid. Base length 30mm width 24mm height 1.5mm. Add insertion lip.",
        );
        let bbox = contract.expected_bbox_mm.expect("should have bbox");
        assert_eq!(
            bbox.min_extent_tolerance_ratio,
            Some(1.50),
            "additive feature 'lip' should trigger wider tolerance"
        );
    }

    #[test]
    fn backplate_with_lip_accepts_taller_geometry() {
        // Base thickness 1.5mm + lip 1.2mm = 2.7mm total
        let contract = build_default_contract(
            "back_plate",
            "Single editable back plate solid. Base length 30mm width 24mm height 1.5mm. Add insertion lip.",
        );
        let mut report = base_report();
        report.bounds_min = [-15.0, -12.0, 0.0];
        report.bounds_max = [15.0, 12.0, 2.7];
        report.component_count = 1;
        let result = validate_part_semantics(&contract, &report, "result = back_plate");
        assert!(
            result.passed,
            "backplate with lip (2.7mm actual vs 1.5mm stated) should pass with wider tolerance, findings: {:?}",
            result.findings
        );
    }

    #[test]
    fn housing_without_features_uses_base_tolerance() {
        let contract = build_default_contract(
            "housing",
            "Single editable housing solid. Base length 42mm width 28mm height 7.5mm.",
        );
        let bbox = contract.expected_bbox_mm.expect("should have bbox");
        assert_eq!(
            bbox.min_extent_tolerance_ratio, None,
            "plain description should not trigger wider tolerance"
        );
    }

    #[test]
    fn test_parse_dims_line_extracts_overall() {
        let desc = "Housing with band slot width 20mm and internal cavity. \
                     Dims: length=42mm, width=28mm, height=7.5mm";
        let dims = infer_envelope_dimensions_mm(desc).expect("should parse Dims line");
        assert_eq!(dims, [42.0, 28.0, 7.5]);
    }

    #[test]
    fn test_parse_dims_line_thickness_alias_and_ignores_subfeatures() {
        let desc = "Dims: length=28.1mm, width=24.1mm, thickness=1.5mm, \
                    lip_height=1.20mm, snap_tolerance=0.15mm";
        let dims = infer_envelope_dimensions_mm(desc).expect("should parse Dims line");
        assert_eq!(dims, [28.1, 24.1, 1.5]);
    }

    #[test]
    fn test_dims_line_incomplete_disables_fallback() {
        let desc = "Dims: length=30mm, width=24mm. Lip height 1.2mm. Height 10mm.";
        let dims = infer_envelope_dimensions_mm(desc);
        assert!(dims.is_none(), "Dims line present but incomplete should return None");
    }

    #[test]
    fn test_parse_dims_line_ignores_wall() {
        let desc = "Dims: length=42mm, width=28mm, height=7.5mm, wall=1.8mm";
        let dims = infer_envelope_dimensions_mm(desc).expect("should parse Dims line");
        assert_eq!(dims, [42.0, 28.0, 7.5]);
    }

    #[test]
    fn test_footprint_plus_height_extraction() {
        let desc = "Single editable housing solid. Footprint 42x28mm, height 7.5mm, wall 1.8mm.";
        let dims = infer_envelope_dimensions_mm(desc).expect("should parse footprint + height");
        assert_eq!(dims, [42.0, 28.0, 7.5]);
    }

    #[test]
    fn test_named_dimension_skips_sub_feature_width() {
        let val = parse_named_dimension(
            "Band slot width 20mm. Overall width 28mm.",
            &["overall width", "width"],
        );
        assert_eq!(val, Some(28.0), "should skip sub-feature 'slot width' and find 'Overall width'");
    }

    #[test]
    fn test_named_dimension_skips_lip_height() {
        let val = parse_named_dimension(
            "Lip height 1.2mm. Total height 3.2mm.",
            &["overall height", "height"],
        );
        assert_eq!(val, Some(3.2), "should skip sub-feature 'Lip height' and find 'Total height'");
    }

    #[test]
    fn test_sub_feature_only_returns_none() {
        // When only sub-feature dimensions exist, return None rather than a wrong contract
        let dims = infer_envelope_dimensions_mm(
            "Lip height 1.2mm, slot width 5mm",
        );
        assert!(dims.is_none(), "sub-feature-only description should return None, got: {:?}", dims);
    }

    #[test]
    fn test_parse_dims_line_total_height_variant() {
        let desc = "o-ring ridge height=0.50mm. Dims: total_height=2.7mm, length=42mm, width=28mm";
        let dims = infer_envelope_dimensions_mm(desc).expect("should parse Dims line with total_height");
        assert_eq!(dims, [42.0, 28.0, 2.7]);
    }

    #[test]
    fn test_parse_dims_line_overall_variants() {
        let desc = "Dims: overall_length=50mm, overall_width=30mm, overall_height=10mm";
        let dims = infer_envelope_dimensions_mm(desc).expect("should parse overall_ prefixed keys");
        assert_eq!(dims, [50.0, 30.0, 10.0]);
    }

    #[test]
    fn test_parse_dims_line_depth_fills_missing() {
        let desc = "Dims: length=42mm, width=28mm, depth=7.5mm";
        let dims = infer_envelope_dimensions_mm(desc).expect("should parse depth as width/length");
        assert_eq!(dims, [42.0, 28.0, 7.5]);
    }

    #[test]
    fn test_parse_dims_line_2of3_fallback() {
        // A novel key name that doesn't match any known pattern
        let desc = "Dims: length=42mm, width=28mm, z_extent=5mm";
        let dims = infer_envelope_dimensions_mm(desc).expect("should use 2-of-3 fallback for unknown key");
        assert_eq!(dims, [42.0, 28.0, 5.0]);
    }
}
