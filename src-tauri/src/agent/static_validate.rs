use regex::Regex;
use serde::Serialize;

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

pub fn validate_code(code: &str) -> StaticValidationResult {
    let mut findings = Vec::new();

    let import_re = Regex::new(r"(?m)^\s*import\s+cadquery\s+as\s+cq\b").unwrap();
    if !import_re.is_match(code) {
        push_error(
            &mut findings,
            "missing_import",
            "Code must include `import cadquery as cq`.",
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

    let translate_bad_sig =
        Regex::new(r"\.translate\s*\(\s*[^\(\)]*?,\s*[^\(\)]*?,\s*[^\(\)]*?\)").unwrap();
    if translate_bad_sig.is_match(code) {
        push_warning(
            &mut findings,
            "translate_signature",
            "`.translate()` should receive a single tuple argument: `.translate((x, y, z))`.",
        );
    }

    let selector_re = Regex::new(r"\.faces\s*\(\s*([^\)]*)\)").unwrap();
    for cap in selector_re.captures_iter(code) {
        let args = cap.get(1).map(|m| m.as_str().trim()).unwrap_or_default();
        if args.contains(',') {
            push_warning(
                &mut findings,
                "faces_selector",
                "`faces()` selector usually expects one selector string or callable; multiple args are risky.",
            );
            break;
        }
    }

    let shell_chain_re = Regex::new(r"(?s)\.(?:cut|union|intersect|fuse)\s*\(.*?\)\s*\.(?:cut|union|intersect|fuse)\s*\(.*?\)\s*\.shell\s*\(").unwrap();
    if shell_chain_re.is_match(code) {
        push_warning(
            &mut findings,
            "shell_after_booleans",
            "`shell()` after multiple booleans is high risk and may fail in OpenCascade.",
        );
    }

    let fillet_chain_re = Regex::new(
        r"(?s)\.(?:cut|union|intersect|fuse)\s*\(.*?\)\s*\.(?:edges\s*\(.*?\)\s*\.)?fillet\s*\(",
    )
    .unwrap();
    if fillet_chain_re.is_match(code) {
        push_warning(
            &mut findings,
            "fillet_after_boolean",
            "Fillet directly after booleans is fragile. Prefer fillet at final stage with conservative radius.",
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

    if code.contains(".cut(")
        && !code.contains(".translate(")
        && !code.contains("workplane(offset=")
        && !code.contains(".pushPoints(")
    {
        push_warning(
            &mut findings,
            "non_intersecting_boolean_risk",
            "Boolean cut detected without obvious placement controls; tool may not intersect target.",
        );
    }

    let passed = findings
        .iter()
        .all(|f| !matches!(f.level, FindingLevel::Error));

    StaticValidationResult { passed, findings }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_validation_success() {
        let code = r#"
import cadquery as cq
result = cq.Workplane("XY").box(10, 10, 10)
"#;
        let result = validate_code(code);
        assert!(result.passed);
    }

    #[test]
    fn test_static_validation_missing_result() {
        let code = "import cadquery as cq\nobj = cq.Workplane('XY').box(1,1,1)";
        let result = validate_code(code);
        assert!(!result.passed);
        assert!(result.findings.iter().any(|f| f.code == "missing_result"));
    }

    #[test]
    fn test_static_validation_detects_file_io() {
        let code = r#"
import cadquery as cq
open("x.txt", "w")
result = cq.Workplane("XY").box(1,1,1)
"#;
        let result = validate_code(code);
        assert!(!result.passed);
        assert!(result.findings.iter().any(|f| f.code == "file_io"));
    }

    #[test]
    fn test_static_validation_warns_non_parametric_hardcoded() {
        let code = r#"
import cadquery as cq
result = cq.Workplane("XY").box(10, 20, 30).faces(">Z").workplane().hole(3).cut(
    cq.Workplane("XY").box(5, 6, 7).translate((1,2,3))
).edges().fillet(0.5)
"#;
        let result = validate_code(code);
        assert!(result
            .findings
            .iter()
            .any(|f| f.code == "non_parametric_hardcoded_dimensions"));
    }
}
