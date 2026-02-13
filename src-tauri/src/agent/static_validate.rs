use regex::Regex;
use serde::Serialize;

use crate::config::GenerationReliabilityProfile;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingLevel {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize)]
pub struct StaticValidationFinding {
    pub level: FindingLevel,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StaticValidationResult {
    pub passed: bool,
    pub findings: Vec<StaticValidationFinding>,
}

fn push_error(findings: &mut Vec<StaticValidationFinding>, code: &str, message: &str) {
    findings.push(StaticValidationFinding {
        level: FindingLevel::Error,
        code: code.to_string(),
        message: message.to_string(),
    });
}

fn push_warning(findings: &mut Vec<StaticValidationFinding>, code: &str, message: &str) {
    findings.push(StaticValidationFinding {
        level: FindingLevel::Warning,
        code: code.to_string(),
        message: message.to_string(),
    });
}

fn push_profile_finding(
    findings: &mut Vec<StaticValidationFinding>,
    profile: &GenerationReliabilityProfile,
    first_pass: bool,
    code: &str,
    message: &str,
) {
    if *profile == GenerationReliabilityProfile::ReliabilityFirst && first_pass {
        push_error(findings, code, message);
    } else {
        push_warning(findings, code, message);
    }
}

pub fn validate_code_with_profile(
    code: &str,
    profile: &GenerationReliabilityProfile,
    first_pass: bool,
) -> StaticValidationResult {
    let mut findings = Vec::new();

    let import_re = Regex::new(r"(?m)^\s*from\s+build123d\s+import\b").unwrap();
    if !import_re.is_match(code) {
        push_error(
            &mut findings,
            "missing_import",
            "Code must include `from build123d import ...`.",
        );
    }

    let result_re = Regex::new(r"(?m)^\s*result\s*=").unwrap();
    if !result_re.is_match(code) {
        push_error(
            &mut findings,
            "missing_result",
            "Code must assign final geometry to `result`.",
        );
    }

    let banned_patterns = [
        (
            r"(?m)\bopen\s*\(",
            "file_io",
            "Direct file I/O is not allowed.",
        ),
        (
            r"(?m)\bos\.",
            "os_access",
            "OS access is not allowed in generated code.",
        ),
        (
            r"(?m)\bsubprocess\b",
            "subprocess",
            "Subprocess execution is not allowed.",
        ),
        (
            r"(?m)\bsocket\b",
            "network_socket",
            "Network access is not allowed in generated code.",
        ),
        (
            r"(?m)\brequests\b|\burllib\b|\bhttpx\b",
            "network_http",
            "HTTP/network libraries are not allowed in generated code.",
        ),
    ];

    for (pat, code_id, msg) in banned_patterns {
        let re = Regex::new(pat).unwrap();
        if re.is_match(code) {
            push_error(&mut findings, code_id, msg);
        }
    }

    let lower = code.to_ascii_lowercase();
    let has_shell = lower.contains("shell(") || lower.contains("offset_3d(");
    let has_loft = lower.contains("loft(");
    let has_fillet_with_edges = lower.contains("fillet(") && lower.contains(".edges()");
    let bool_re = Regex::new(r"(?:\.(cut|union|intersect|fuse|combine)\s*\(|\s-\s)").unwrap();
    let boolean_count = bool_re.find_iter(&lower).count();

    let shell_chain_re = Regex::new(r"(?s)\.(?:cut|union|intersect|fuse|combine)\s*\(.*?\)\s*\.(?:cut|union|intersect|fuse|combine)\s*\(.*?\)\s*\.shell\s*\(").unwrap();
    if shell_chain_re.is_match(code) || (has_shell && boolean_count >= 2) {
        push_profile_finding(
            &mut findings,
            profile,
            first_pass,
            "shell_after_booleans",
            "`shell()` after multi-boolean chains is fragile; prefer inner-solid subtraction.",
        );
    }

    if has_loft && has_shell {
        push_profile_finding(
            &mut findings,
            profile,
            first_pass,
            "loft_shell_combo",
            "Using `loft()` and `shell()` together in first-pass generation is a known reliability risk.",
        );
    }

    if has_fillet_with_edges && (has_loft || has_shell || boolean_count >= 2) {
        push_profile_finding(
            &mut findings,
            profile,
            first_pass,
            "blanket_fillet_on_complex_body",
            "Blanket `.edges().fillet()/chamfer()` on loft/shell/multi-boolean geometry is high risk.",
        );
    }

    let num_re = Regex::new(r"\b\d+(?:\.\d+)?\b").unwrap();
    let uppercase_param_re = Regex::new(r"(?m)^\s*[A-Z][A-Z0-9_]*\s*=").unwrap();
    let numeric_count = num_re.find_iter(code).count();
    if numeric_count > 10 && !uppercase_param_re.is_match(code) {
        push_warning(
            &mut findings,
            "non_parametric_hardcoded_dimensions",
            "Many hardcoded numeric literals detected without named parameter constants.",
        );
    }

    if (code.contains(".cut(") || code.contains(" - "))
        && !code.contains("Pos(")
        && !code.contains("Location(")
        && !code.contains("Plane(")
    {
        push_warning(
            &mut findings,
            "non_intersecting_boolean_risk",
            "Boolean cut detected without obvious placement controls; tool may not intersect target.",
        );
    }

    let mentions_mechanism = [
        "snap", "hinge", "boss", "gasket", "o_ring", "oring", "detent", "bayonet", "dovetail",
    ]
    .iter()
    .any(|k| lower.contains(k));
    let has_tolerance_var = lower.contains("clearance")
        || lower.contains("tolerance")
        || lower.contains("gap")
        || lower.contains("fit_delta");
    if mentions_mechanism && !has_tolerance_var {
        push_warning(
            &mut findings,
            "mechanism_tolerance_missing",
            "Mechanism-like geometry detected without explicit tolerance/clearance variables.",
        );
    }

    let passed = findings
        .iter()
        .all(|f| !matches!(f.level, FindingLevel::Error));

    StaticValidationResult { passed, findings }
}

pub fn validate_code(code: &str) -> StaticValidationResult {
    validate_code_with_profile(code, &GenerationReliabilityProfile::Balanced, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GenerationReliabilityProfile;

    #[test]
    fn test_static_validation_success() {
        let code = r#"
from build123d import *
result = Box(10, 10, 10)
"#;
        let result = validate_code(code);
        assert!(result.passed);
    }

    #[test]
    fn test_static_validation_missing_result() {
        let code = "from build123d import *\nobj = Box(1, 1, 1)";
        let result = validate_code(code);
        assert!(!result.passed);
        assert!(result.findings.iter().any(|f| f.code == "missing_result"));
    }

    #[test]
    fn test_static_validation_detects_file_io() {
        let code = r#"
from build123d import *
open("x.txt", "w")
result = Box(1, 1, 1)
"#;
        let result = validate_code(code);
        assert!(!result.passed);
        assert!(result.findings.iter().any(|f| f.code == "file_io"));
    }

    #[test]
    fn test_static_validation_warns_non_parametric_hardcoded() {
        let code = r#"
from build123d import *
with BuildPart() as p:
    Box(10, 20, 30)
    with Locations((1, 2, 3)):
        Box(5, 6, 7, mode=Mode.SUBTRACT)
    with Locations((15, 25, 8)):
        Cylinder(4, 12, mode=Mode.SUBTRACT)
    fillet(p.edges(), radius=0.5)
result = p.part
"#;
        let result = validate_code(code);
        assert!(result
            .findings
            .iter()
            .any(|f| f.code == "non_parametric_hardcoded_dimensions"));
    }

    #[test]
    fn test_reliability_first_escalates_loft_shell_combo() {
        let code = r#"
from build123d import *
with BuildPart() as p:
    with BuildSketch():
        Rectangle(10, 10)
    with BuildSketch(Plane.XY.offset(5)):
        Rectangle(8, 8)
    loft()
    offset_3d(openings=p.faces().sort_by(Axis.Z)[-1], amount=-1)
result = p.part
"#;
        let result =
            validate_code_with_profile(code, &GenerationReliabilityProfile::ReliabilityFirst, true);
        assert!(!result.passed);
        assert!(result.findings.iter().any(|f| f.code == "loft_shell_combo"));
    }

    #[test]
    fn test_balanced_keeps_loft_shell_as_warning() {
        let code = r#"
from build123d import *
with BuildPart() as p:
    with BuildSketch():
        Rectangle(10, 10)
    with BuildSketch(Plane.XY.offset(5)):
        Rectangle(8, 8)
    loft()
    offset_3d(openings=p.faces().sort_by(Axis.Z)[-1], amount=-1)
result = p.part
"#;
        let result =
            validate_code_with_profile(code, &GenerationReliabilityProfile::Balanced, true);
        assert!(result.findings.iter().any(|f| f.code == "loft_shell_combo"));
        assert!(result
            .findings
            .iter()
            .any(|f| matches!(f.level, FindingLevel::Warning)));
    }
}
