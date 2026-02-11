use regex::Regex;
use serde::Serialize;

use crate::ai::message::ChatMessage;
use crate::ai::provider::{AiProvider, TokenUsage};
use crate::error::AppError;

/// The geometry design plan produced by the advisor before code generation.
#[derive(Debug, Clone)]
pub struct DesignPlan {
    pub text: String,
}

/// Result of deterministic plan validation (no AI calls).
#[derive(Debug, Clone, Serialize)]
pub struct PlanValidation {
    pub is_valid: bool,
    pub risk_score: u32,
    pub warnings: Vec<String>,
    pub rejected_reason: Option<String>,
    pub extracted_operations: Vec<String>,
    pub extracted_dimensions: Vec<f64>,
    pub plan_text: String,
}

const GEOMETRY_ADVISOR_PROMPT: &str = r#"You are a CAD geometry planner. Your job is to analyze a user's request and produce a detailed geometric build plan BEFORE any code is written.

You must think carefully about what the object actually looks like and how to build it with CadQuery primitives. Do NOT write any code — describe the geometry.
Do not include hidden reasoning, self-critique, or internal deliberation in the output.
Output only the final plan sections listed below.

## Your Output Format

### Object Analysis
Describe what this object looks like in the real world. What are its key visual features? What are its proportions? What makes it recognizable?

### CadQuery Approach
Which CadQuery primitives and operations best approximate each feature? Be specific:
- For axially symmetric shapes → revolve() with a spline/polyline profile
- For shapes that vary along a height → loft() between profiles at different heights
- For shapes with a constant cross-section along a path → sweep()
- For mechanical parts → boxes, cylinders, and boolean operations
- For organic curves → spline profiles with revolve/loft, generous fillets

### Build Plan
Number each step. Be specific about dimensions and positions:
1. Start with [base shape] — dimensions: X×Y×Z mm
2. Add [feature] using [operation] — positioned at (x, y, z)
3. Cut [opening] using [method] — dimensions and position
...

### Approximation Notes
What can't CadQuery do perfectly? What's the closest buildable shape? Where should fillets be applied to smooth transitions?

## Rules
- Think about CROSS-SECTIONS: describe the profile shape at key heights
- Think about PROPORTIONS: a helmet is roughly 200mm tall, a phone is ~150mm long, etc.
- Think about what makes the object RECOGNIZABLE — which features are essential vs decorative
- For organic shapes: plan by cross-section at multiple heights, then use loft or revolve
- For mechanical parts: plan by feature (base → holes → slots → fillets)
- Prefer approaches that are ROBUST in CadQuery (box+cylinder+booleans > complex lofts)
- If the request is simple (e.g. "a box" or "a cylinder"), keep the plan brief — 2-3 lines is fine
- NEVER write Python or CadQuery code — only describe geometry in plain English"#;

// ---------------------------------------------------------------------------
// Manufacturing constraints formatting
// ---------------------------------------------------------------------------

/// Recursively format a YAML value into indented bullet-point markdown.
fn format_yaml_value(out: &mut String, value: &serde_yaml::Value, indent: usize) {
    let prefix = "  ".repeat(indent);
    match value {
        serde_yaml::Value::Mapping(map) => {
            for (k, v) in map {
                let key = match k {
                    serde_yaml::Value::String(s) => s.clone(),
                    other => format!("{:?}", other),
                };
                match v {
                    serde_yaml::Value::Mapping(_) | serde_yaml::Value::Sequence(_) => {
                        out.push_str(&format!("{}- **{}**:\n", prefix, key));
                        format_yaml_value(out, v, indent + 1);
                    }
                    _ => {
                        let val = format_yaml_scalar(v);
                        out.push_str(&format!("{}- **{}**: {}\n", prefix, key, val));
                    }
                }
            }
        }
        serde_yaml::Value::Sequence(seq) => {
            for item in seq {
                match item {
                    serde_yaml::Value::Mapping(_) | serde_yaml::Value::Sequence(_) => {
                        out.push_str(&format!("{}- \n", prefix));
                        format_yaml_value(out, item, indent + 1);
                    }
                    _ => {
                        let val = format_yaml_scalar(item);
                        out.push_str(&format!("{}- {}\n", prefix, val));
                    }
                }
            }
        }
        _ => {
            let val = format_yaml_scalar(value);
            out.push_str(&format!("{}{}\n", prefix, val));
        }
    }
}

/// Format a scalar YAML value as a string.
fn format_yaml_scalar(value: &serde_yaml::Value) -> String {
    match value {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Null => "null".to_string(),
        _ => format!("{:?}", value),
    }
}

/// Format manufacturing constraints from AgentRules into a prompt-ready string.
/// Returns a markdown section that can be appended to the geometry advisor prompt.
pub fn format_manufacturing_constraints(manufacturing: &serde_yaml::Value) -> String {
    let mut out = String::new();
    out.push_str("## Manufacturing Constraints\n");
    out.push_str("The user's active manufacturing profile imposes these constraints. ");
    out.push_str("Your geometry plan MUST respect them.\n\n");
    format_yaml_value(&mut out, manufacturing, 0);
    out
}

/// Format dimension guidance rules into a prompt-ready string for the geometry advisor.
pub fn format_dimension_guidance(
    guidance: &std::collections::HashMap<String, Vec<String>>,
) -> String {
    let mut out = String::new();
    out.push_str("## Dimension Estimation Guidance\n");
    out.push_str("Follow these rules when choosing dimensions for your geometry plan.\n\n");
    for (category, items) in guidance {
        let title: String = category
            .split('_')
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        out.push_str(&format!("### {}\n", title));
        for item in items {
            out.push_str(&format!("- {}\n", item));
        }
        out.push('\n');
    }
    out
}

/// Format failure prevention rules into a prompt-ready string for the geometry advisor.
pub fn format_failure_prevention(
    prevention: &std::collections::HashMap<String, Vec<String>>,
) -> String {
    let mut out = String::new();
    out.push_str("## Failure Prevention Rules\n");
    out.push_str(
        "Follow these rules when planning geometry to avoid operations that commonly fail.\n\n",
    );
    for (category, items) in prevention {
        let title: String = category
            .split('_')
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        out.push_str(&format!("### {}\n", title));
        for item in items {
            out.push_str(&format!("- {}\n", item));
        }
        out.push('\n');
    }
    out
}

// ---------------------------------------------------------------------------
// Parsing helpers (private)
// ---------------------------------------------------------------------------

const KNOWN_OPERATIONS: &[&str] = &[
    "fillet",
    "chamfer",
    "shell",
    "loft",
    "sweep",
    "revolve",
    "cut",
    "union",
    "intersect",
    "extrude",
    "hole",
    "fuse",
    "combine",
];

const BOOLEAN_OPS: &[&str] = &["cut", "union", "fuse", "intersect", "combine"];

/// Check if "shell" appears as a CadQuery operation (verb) rather than an English noun.
/// Matches: "shell(", "shell it", "shell the body", "apply shell", "use shell", "then shell"
/// Does NOT match: "hollow shell", "outer shell", "thick shell"
fn has_shell_operation(text: &str) -> bool {
    let lower = text.to_lowercase();
    // Code/function call syntax: shell( or shell ()
    if Regex::new(r"shell\s*\(").unwrap().is_match(&lower) {
        return true;
    }
    // Imperative verb: "shell it", "shell the body", "shell this"
    if Regex::new(r"\bshell\s+(?:it|the|this|that|a|an)\b")
        .unwrap()
        .is_match(&lower)
    {
        return true;
    }
    // Preceded by action verbs: "apply shell", "use shell", "then shell", "and shell"
    if Regex::new(r"(?:apply|use|then|and|perform)\s+shell\b")
        .unwrap()
        .is_match(&lower)
    {
        return true;
    }
    false
}

/// Extract known CadQuery operation names from the plan text (unique, case-insensitive).
fn extract_operations(plan_text: &str) -> Vec<String> {
    let lower = plan_text.to_lowercase();
    let mut ops = Vec::new();
    for &op in KNOWN_OPERATIONS {
        if op == "shell" {
            // Special handling: "shell" as noun is very common in CAD prose
            if has_shell_operation(plan_text) {
                ops.push(op.to_string());
            }
        } else {
            let pattern = format!(r"\b{}\b", op);
            if let Ok(re) = Regex::new(&pattern) {
                if re.is_match(&lower) {
                    ops.push(op.to_string());
                }
            }
        }
    }
    ops
}

/// Extract numeric dimensions (in mm) from the plan text.
fn extract_dimensions(plan_text: &str) -> Vec<f64> {
    let mut dims = Vec::new();
    let mut multi_dim_values = std::collections::HashSet::new();

    // Pattern 1: multi-dim like "50x30x20mm" or "50 x 30 x 20 mm"
    let multi_re =
        Regex::new(r"(-?\d+\.?\d*)\s*[x×]\s*(-?\d+\.?\d*)(?:\s*[x×]\s*(-?\d+\.?\d*))?\s*(?:mm)?\b")
            .unwrap();
    for cap in multi_re.captures_iter(plan_text) {
        if let Ok(v) = cap[1].parse::<f64>() {
            dims.push(v);
            multi_dim_values.insert(cap[1].to_string());
        }
        if let Ok(v) = cap[2].parse::<f64>() {
            dims.push(v);
            multi_dim_values.insert(cap[2].to_string());
        }
        if let Some(m) = cap.get(3) {
            if let Ok(v) = m.as_str().parse::<f64>() {
                dims.push(v);
                multi_dim_values.insert(m.as_str().to_string());
            }
        }
    }

    // Pattern 2: single like "5mm", "100 mm", "-2mm"
    let single_re = Regex::new(r"(-?\d+\.?\d*)\s*mm\b").unwrap();
    for cap in single_re.captures_iter(plan_text) {
        let val_str = &cap[1];
        // Skip if already captured by multi-dim pattern
        if multi_dim_values.contains(val_str) {
            continue;
        }
        if let Ok(v) = val_str.parse::<f64>() {
            // Skip negative values in offset/position context
            if v < 0.0 {
                let match_start = cap.get(1).unwrap().start();
                if is_offset_context(plan_text, match_start) {
                    continue;
                }
            }
            dims.push(v);
        }
    }

    dims
}

/// Extract fillet radii mentioned near "fillet" keywords.
fn extract_fillet_radii(plan_text: &str) -> Vec<f64> {
    let re = Regex::new(r"(?i)fillet\w*[\s\(\-\x{2014}:]+(\d+\.?\d*)\s*(?:mm)?").unwrap();
    let mut radii = Vec::new();
    for cap in re.captures_iter(plan_text) {
        if let Ok(v) = cap[1].parse::<f64>() {
            radii.push(v);
        }
    }
    radii
}

/// Extract chamfer sizes mentioned near "chamfer" keywords.
fn extract_chamfer_sizes(plan_text: &str) -> Vec<f64> {
    let re = Regex::new(r"(?i)chamfer\w*[\s\(\-\x{2014}:]+(\d+\.?\d*)\s*(?:mm)?").unwrap();
    let mut sizes = Vec::new();
    for cap in re.captures_iter(plan_text) {
        if let Ok(v) = cap[1].parse::<f64>() {
            sizes.push(v);
        }
    }
    sizes
}

/// Check whether a negative number at `match_start` is in an offset/position context
/// (the same line contains words like "offset", "move", "translate", etc.).
fn is_offset_context(plan_text: &str, match_start: usize) -> bool {
    const OFFSET_WORDS: &[&str] = &[
        "offset",
        "position",
        "move",
        "translate",
        "adjust",
        "shift",
        "back",
        "direction",
        "from",
        "away",
    ];

    let line_start = plan_text[..match_start]
        .rfind('\n')
        .map(|p| p + 1)
        .unwrap_or(0);

    let line_prefix = &plan_text[line_start..match_start];
    let lower = line_prefix.to_lowercase();

    OFFSET_WORDS.iter().any(|w| lower.contains(w))
}

/// Extract the body text of a markdown section by heading name.
/// Returns text between the matched heading and the next heading (or EOF).
fn extract_section(plan_text: &str, heading: &str) -> Option<String> {
    let lower = plan_text.to_lowercase();
    let heading_lower = heading.to_lowercase();

    let patterns = [
        format!("### {}", heading_lower),
        format!("## {}", heading_lower),
    ];

    let mut section_start = None;
    for pattern in &patterns {
        if let Some(pos) = lower.find(pattern.as_str()) {
            let after_heading = pos + pattern.len();
            section_start = Some(
                plan_text[after_heading..]
                    .find('\n')
                    .map(|p| after_heading + p + 1)
                    .unwrap_or(plan_text.len()),
            );
            break;
        }
    }

    let start = section_start?;
    if start >= plan_text.len() {
        return None;
    }

    // Find the next section heading (## or ###)
    let rest = &plan_text[start..];
    let heading_re = Regex::new(r"(?m)^#{2,3}\s+\S").unwrap();
    let end = heading_re
        .find(rest)
        .map(|m| start + m.start())
        .unwrap_or(plan_text.len());

    let body = plan_text[start..end].trim().to_string();
    if body.is_empty() {
        None
    } else {
        Some(body)
    }
}

/// Extract only numbered build-step lines from the Build Plan section.
/// Returns None if there is no Build Plan section or no numbered steps.
fn extract_build_plan_steps_text(plan_text: &str) -> Option<String> {
    let section = extract_section(plan_text, "Build Plan")?;
    let step_re = Regex::new(r"^\s*\d+\.\s+.+").unwrap();
    let steps: Vec<String> = section
        .lines()
        .filter(|line| step_re.is_match(line))
        .map(|line| line.trim().to_string())
        .collect();
    if steps.is_empty() {
        None
    } else {
        Some(steps.join("\n"))
    }
}

/// Remove any preamble text before the first expected section heading.
/// This prevents leaked internal reasoning from polluting validators and UI.
fn sanitize_plan_text(plan_text: &str) -> String {
    // First try to normalize loosely formatted section labels (e.g. "*Object Analysis*:")
    // into canonical markdown headings.
    if let Some(normalized) = normalize_section_labels(plan_text) {
        return normalized;
    }

    let heading_re =
        Regex::new(r"(?im)^#{2,3}\s+(object analysis|cadquery approach|build plan|approximation notes)\s*$")
            .unwrap();
    if let Some(m) = heading_re.find(plan_text) {
        plan_text[m.start()..].trim().to_string()
    } else {
        plan_text.trim().to_string()
    }
}

/// Normalize loose section label styles into canonical markdown headings.
///
/// Accepts variants like:
/// - "*Object Analysis*:"
/// - "**CadQuery Approach**"
/// - "Build Plan Structure:"
/// - "Object Analysis:"
///
/// Returns None if no recognizable section labels are found.
fn normalize_section_labels(plan_text: &str) -> Option<String> {
    fn canonical_label(line: &str) -> Option<&'static str> {
        let mut s = line.trim();
        if s.is_empty() {
            return None;
        }

        // Drop common markdown/list prefixes
        while let Some(first) = s.chars().next() {
            if first == '#' || first == '-' || first == '*' || first == ' ' {
                s = s[1..].trim_start();
            } else {
                break;
            }
        }

        // Drop markdown emphasis and trailing punctuation
        let cleaned = s
            .replace('*', "")
            .trim()
            .trim_end_matches(':')
            .trim()
            .to_lowercase();

        if cleaned == "object analysis" {
            Some("Object Analysis")
        } else if cleaned == "cadquery approach" {
            Some("CadQuery Approach")
        } else if cleaned == "build plan" || cleaned == "build plan structure" {
            Some("Build Plan")
        } else if cleaned == "approximation notes" {
            Some("Approximation Notes")
        } else {
            None
        }
    }

    let mut current: Option<&'static str> = None;
    let mut sections: std::collections::HashMap<&'static str, Vec<String>> =
        std::collections::HashMap::new();
    let mut seen_any = false;

    for raw_line in plan_text.lines() {
        if let Some(lbl) = canonical_label(raw_line) {
            current = Some(lbl);
            seen_any = true;
            sections.entry(lbl).or_default();
            continue;
        }

        if let Some(lbl) = current {
            sections.entry(lbl).or_default().push(raw_line.to_string());
        }
    }

    if !seen_any {
        return None;
    }

    let order = [
        "Object Analysis",
        "CadQuery Approach",
        "Build Plan",
        "Approximation Notes",
    ];

    let mut out = String::new();
    for name in order {
        if let Some(lines) = sections.get(name) {
            if lines.is_empty() {
                continue;
            }
            let body = lines.join("\n").trim().to_string();
            if body.is_empty() {
                continue;
            }
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&format!("### {}\n{}", name, body));
        }
    }

    if out.trim().is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Count total mentions of boolean operations (not unique — repeated mentions count).
fn count_boolean_mentions(plan_text: &str) -> usize {
    let lower = plan_text.to_lowercase();
    let mut count = 0;
    for &op in BOOLEAN_OPS {
        let pattern = format!(r"\b{}\b", op);
        if let Ok(re) = Regex::new(&pattern) {
            count += re.find_iter(&lower).count();
        }
    }
    count
}

/// Count boolean mentions scoped to the Build Plan section only.
/// Falls back to the full text if no Build Plan section is found.
#[cfg(test)]
fn count_boolean_mentions_in_build_plan(plan_text: &str) -> usize {
    let section_text =
        extract_section(plan_text, "Build Plan").unwrap_or_else(|| plan_text.to_string());
    count_boolean_mentions(&section_text)
}

/// Check whether the plan contains a section heading (case-insensitive).
fn has_section(plan_text: &str, heading: &str) -> bool {
    let lower = plan_text.to_lowercase();
    let heading_lower = heading.to_lowercase();
    // Match "### Build Plan", "## Build Plan", or just "Build Plan" on its own line
    lower.contains(&format!("### {}", heading_lower))
        || lower.contains(&format!("## {}", heading_lower))
        || lower
            .lines()
            .any(|line| line.trim().to_lowercase() == heading_lower)
}

/// Public wrapper for `extract_operations` — used by `iterative.rs` to detect risky ops.
pub fn extract_operations_from_text(text: &str) -> Vec<String> {
    extract_operations(text)
}

// ---------------------------------------------------------------------------
// Plan validation
// ---------------------------------------------------------------------------

/// Validate a design plan deterministically (no AI calls).
///
/// Extracts operations and dimensions, calculates a risk score (0-10),
/// and rejects plans with a score > 7.
pub fn validate_plan(plan_text: &str) -> PlanValidation {
    let sanitized = sanitize_plan_text(plan_text);
    let plan_text = sanitized.as_str();

    // Scope operation/risk extraction to numbered Build Plan steps only.
    let build_plan_steps_text = extract_build_plan_steps_text(plan_text);
    let operations = build_plan_steps_text
        .as_ref()
        .map(|s| extract_operations(s))
        .unwrap_or_default();

    // Scope dimensional risk checks to numbered Build Plan steps only.
    // This avoids false negatives/positives from prose or leaked "thinking" text.
    let dimensions = build_plan_steps_text
        .as_ref()
        .map(|s| extract_dimensions(s))
        .unwrap_or_default();
    let fillet_radii = extract_fillet_radii(plan_text);
    let chamfer_sizes = extract_chamfer_sizes(plan_text);
    // Scope boolean counting to numbered Build Plan steps only
    let boolean_count = build_plan_steps_text
        .as_ref()
        .map(|s| count_boolean_mentions(s))
        .unwrap_or(0);

    let has_shell = operations.contains(&"shell".to_string());
    let has_loft = operations.contains(&"loft".to_string());
    let has_sweep = operations.contains(&"sweep".to_string());
    let has_revolve = operations.contains(&"revolve".to_string());
    let has_chamfer = operations.contains(&"chamfer".to_string());

    let min_positive_dimension = dimensions
        .iter()
        .copied()
        .filter(|&d| d > 0.0)
        .reduce(f64::min);

    let mut risk: u32 = 0;
    let mut warnings: Vec<String> = Vec::new();

    // Rule 1: shell after many booleans
    if has_shell && boolean_count > 3 {
        risk += 3;
        warnings.push(format!(
            "shell() after {} boolean operations is very likely to fail",
            boolean_count
        ));
    }

    // Rule 2: large fillet on small feature
    if let Some(min_dim) = min_positive_dimension {
        if min_dim < 20.0 {
            for &r in &fillet_radii {
                if r > 5.0 {
                    risk += 2;
                    warnings.push(format!(
                        "fillet radius {}mm may be too large for features as small as {}mm",
                        r, min_dim
                    ));
                    break; // only count once
                }
            }
        }
    }

    // Rule 3: many boolean operations
    if boolean_count >= 5 {
        risk += 2;
        warnings.push(format!(
            "{} boolean operations increases topology failure risk",
            boolean_count
        ));
    }

    // Rule 4: loft is fragile
    if has_loft {
        risk += 1;
        warnings
            .push("loft() is fragile — ensure profiles have compatible edge counts".to_string());
    }

    // Rule 5: sweep requires wire
    if has_sweep {
        risk += 1;
        warnings.push("sweep() requires a Wire path — ensure .wire() is called".to_string());
    }

    // Rule 6: revolve axis
    if has_revolve {
        risk += 1;
        warnings.push(
            "revolve() — ensure profile is entirely on one side of rotation axis".to_string(),
        );
    }

    // Rule 7: negative dimensions
    for &d in &dimensions {
        if d < 0.0 {
            risk += 4;
            warnings.push(format!(
                "negative dimension {}mm is physically impossible",
                d
            ));
            break; // only count once
        }
    }

    // Rule 8: huge dimensions
    for &d in &dimensions {
        if d > 10000.0 {
            risk += 3;
            warnings.push(format!(
                "dimension {}mm exceeds 10 meters — likely a mistake",
                d
            ));
            break;
        }
    }

    // Rule 9: tiny dimensions
    for &d in &dimensions {
        if d > 0.0 && d < 0.01 {
            risk += 3;
            warnings.push(format!(
                "dimension {}mm is below manufacturing precision",
                d
            ));
            break;
        }
    }

    // Rule 10: missing Build Plan section
    if !has_section(plan_text, "Build Plan") {
        risk += 2;
        warnings.push("plan is missing a 'Build Plan' section with numbered steps".to_string());
    }
    // Rule 10b: Build Plan exists but no numbered steps
    if has_section(plan_text, "Build Plan") && build_plan_steps_text.is_none() {
        risk += 2;
        warnings.push("Build Plan section must include numbered steps (1., 2., 3., ...)".to_string());
    }

    // Rule 11: no dimensions
    if dimensions.is_empty() {
        risk += 2;
        warnings.push("no concrete dimensions found in plan".to_string());
    }

    // Rule 12: no operations
    if operations.is_empty() {
        risk += 1;
        warnings.push("no CadQuery operations mentioned in plan".to_string());
    }

    // Rule 13: shell with thin walls (skip dimensions that are fillet radii or chamfer sizes)
    if has_shell {
        for &d in &dimensions {
            if d > 0.0 && d < 2.0 {
                let is_fillet = fillet_radii.iter().any(|&r| (r - d).abs() < 0.001);
                let is_chamfer = chamfer_sizes.iter().any(|&c| (c - d).abs() < 0.001);
                if !is_fillet && !is_chamfer {
                    risk += 2;
                    warnings.push(format!(
                        "shell() with wall thickness {}mm may fail in OpenCascade",
                        d
                    ));
                    break;
                }
            }
        }
    }

    // Rule 14: chamfer after many booleans
    if has_chamfer && boolean_count > 3 {
        risk += 2;
        warnings.push(format!(
            "chamfer() after {} boolean operations is risky",
            boolean_count
        ));
    }

    // Rule 15: missing Object Analysis section
    if !has_section(plan_text, "Object Analysis") {
        risk += 1;
        warnings.push("plan is missing an 'Object Analysis' section".to_string());
    }

    // Rule 16: missing CadQuery Approach section
    if !has_section(plan_text, "CadQuery Approach") {
        risk += 1;
        warnings.push("plan is missing a 'CadQuery Approach' section".to_string());
    }

    // Clamp to 10
    risk = risk.min(10);

    let is_valid = risk <= 7;
    let rejected_reason = if !is_valid {
        warnings.first().cloned()
    } else {
        None
    };

    PlanValidation {
        is_valid,
        risk_score: risk,
        warnings,
        rejected_reason,
        extracted_operations: operations,
        extracted_dimensions: dimensions,
        plan_text: plan_text.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Feedback and re-prompt
// ---------------------------------------------------------------------------

/// Build a human-readable feedback message for a rejected plan.
pub fn build_rejection_feedback(validation: &PlanValidation) -> String {
    let mut feedback = format!(
        "## Plan Validation Feedback\nYour previous plan was rejected (risk score {}/10).\n\n",
        validation.risk_score
    );

    if let Some(ref reason) = validation.rejected_reason {
        feedback.push_str(&format!("**Primary concern:** {}\n\n", reason));
    }

    if !validation.warnings.is_empty() {
        feedback.push_str("**All warnings:**\n");
        for w in &validation.warnings {
            feedback.push_str(&format!("- {}\n", w));
        }
        feedback.push('\n');
    }

    feedback.push_str(
        "**Instructions for revision:**\n\
         - Use simpler operations where possible (prefer box+cylinder+booleans over complex lofts)\n\
         - Use realistic, achievable dimensions\n\
         - Apply fillets and chamfers as the LAST operations\n\
         - Include a '### Build Plan' section with numbered steps\n"
    );

    feedback
}

/// Re-call the geometry advisor with feedback about a rejected plan.
pub async fn plan_geometry_with_feedback(
    provider: Box<dyn AiProvider>,
    user_request: &str,
    feedback: &str,
    manufacturing_context: Option<&str>,
) -> Result<(DesignPlan, Option<TokenUsage>), AppError> {
    let mut system_prompt = GEOMETRY_ADVISOR_PROMPT.to_string();
    if let Some(ctx) = manufacturing_context {
        system_prompt.push_str("\n\n");
        system_prompt.push_str(ctx);
    }

    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: system_prompt,
        },
        ChatMessage {
            role: "user".to_string(),
            content: user_request.to_string(),
        },
        ChatMessage {
            role: "assistant".to_string(),
            content: "(Previous plan was rejected by the validator.)".to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: format!(
                "{}\n\nPlease generate a REVISED geometry plan that addresses these concerns.",
                feedback
            ),
        },
    ];

    let (plan_text, usage) = provider.complete(&messages, Some(2048)).await?;
    Ok((
        DesignPlan {
            text: sanitize_plan_text(&plan_text),
        },
        usage,
    ))
}

/// Call the AI to produce a geometry design plan for the user's request.
///
/// This is the "design-first" phase that runs before code generation,
/// giving the code generator concrete geometric instructions instead of
/// a vague natural-language description.
pub async fn plan_geometry(
    provider: Box<dyn AiProvider>,
    user_request: &str,
    manufacturing_context: Option<&str>,
) -> Result<(DesignPlan, Option<TokenUsage>), AppError> {
    let mut system_prompt = GEOMETRY_ADVISOR_PROMPT.to_string();
    if let Some(ctx) = manufacturing_context {
        system_prompt.push_str("\n\n");
        system_prompt.push_str(ctx);
    }

    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: system_prompt,
        },
        ChatMessage {
            role: "user".to_string(),
            content: user_request.to_string(),
        },
    ];

    // Use complete (non-streaming) since the plan is relatively short
    // and we want the full text before proceeding to code generation.
    let (plan_text, usage) = provider.complete(&messages, Some(2048)).await?;

    Ok((
        DesignPlan {
            text: sanitize_plan_text(&plan_text),
        },
        usage,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geometry_advisor_prompt_content() {
        assert!(GEOMETRY_ADVISOR_PROMPT.contains("geometry planner"));
        assert!(GEOMETRY_ADVISOR_PROMPT.contains("Build Plan"));
        assert!(GEOMETRY_ADVISOR_PROMPT.contains("Object Analysis"));
        assert!(GEOMETRY_ADVISOR_PROMPT.contains("CadQuery Approach"));
        assert!(GEOMETRY_ADVISOR_PROMPT.contains("NEVER write Python"));
    }

    #[test]
    fn test_design_plan_struct() {
        let plan = DesignPlan {
            text: "Test plan".to_string(),
        };
        assert_eq!(plan.text, "Test plan");
    }

    // -----------------------------------------------------------------------
    // Parsing tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_operations_finds_known_ops() {
        let text = "We will revolve a profile, then shell it, and add a fillet.";
        let ops = extract_operations(text);
        assert!(ops.contains(&"revolve".to_string()));
        assert!(ops.contains(&"shell".to_string()));
        assert!(ops.contains(&"fillet".to_string()));
    }

    #[test]
    fn test_extract_operations_empty_for_no_ops() {
        let text = "This is a plain description of a rectangular object.";
        let ops = extract_operations(text);
        assert!(ops.is_empty());
    }

    #[test]
    fn test_extract_dimensions_multi_format() {
        let text = "Start with a box 50x30x20mm.";
        let dims = extract_dimensions(text);
        assert!(dims.contains(&50.0));
        assert!(dims.contains(&30.0));
        assert!(dims.contains(&20.0));
    }

    #[test]
    fn test_extract_dimensions_single_format() {
        let text = "Use a 3mm fillet and a 100 mm radius.";
        let dims = extract_dimensions(text);
        assert!(dims.contains(&3.0));
        assert!(dims.contains(&100.0));
    }

    #[test]
    fn test_extract_dimensions_negative() {
        // Non-offset context: negative dimension is a genuine error
        let text = "Create a box with -2mm height.";
        let dims = extract_dimensions(text);
        assert!(dims.contains(&-2.0));
    }

    #[test]
    fn test_extract_fillet_radii() {
        let text = "Apply fillet 5mm on top edges and fillet(2.0) on bottom.";
        let radii = extract_fillet_radii(text);
        assert!(radii.contains(&5.0));
        assert!(radii.contains(&2.0));
    }

    #[test]
    fn test_count_boolean_mentions_multiple() {
        let text = "Cut the slot, cut the hole, union the boss, fuse the cap.";
        let count = count_boolean_mentions(text);
        assert_eq!(count, 4);
    }

    #[test]
    fn test_has_section_present() {
        let text = "Some intro\n### Build Plan\n1. Start with a box";
        assert!(has_section(text, "Build Plan"));
    }

    #[test]
    fn test_has_section_missing() {
        let text = "Some intro\n### Object Analysis\nA box.";
        assert!(!has_section(text, "Build Plan"));
    }

    // -----------------------------------------------------------------------
    // Risk score tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_validate_simple_plan_low_risk() {
        let text = "### Object Analysis\nA simple rectangular box.\n\n\
            ### CadQuery Approach\nUse extrude to create the base.\n\n\
            ### Build Plan\n1. Extrude a 50x30x20mm box.";
        let v = validate_plan(text);
        assert!(v.is_valid);
        assert!(v.risk_score <= 3, "risk {} should be <= 3", v.risk_score);
    }

    #[test]
    fn test_validate_shell_after_many_booleans() {
        let text = "### Build Plan\n\
            Dimensions: 100x80x50mm\n\
            1. Cut a slot. 2. Cut a hole. 3. Cut another slot. 4. Cut a pocket. 5. Cut a vent.\n\
            6. Apply shell to hollow it out.";
        let v = validate_plan(text);
        // Rule 1: shell + 5 booleans = +3, Rule 3: 5 booleans = +2 → score >= 5
        assert!(v.risk_score >= 5);
        assert!(v.warnings.iter().any(|w| w.contains("shell()")));
    }

    #[test]
    fn test_extract_operations_shell_noun_not_detected() {
        // "shell" used as an English noun should NOT be detected as a CadQuery operation
        let text = "Boolean subtract inner solid from outer solid → 3mm thick hollow shell";
        let ops = extract_operations(text);
        assert!(
            !ops.contains(&"shell".to_string()),
            "'hollow shell' (noun) should not be detected as shell operation"
        );

        let text2 = "The outer shell profile is smooth.";
        let ops2 = extract_operations(text2);
        assert!(
            !ops2.contains(&"shell".to_string()),
            "'outer shell' (noun) should not be detected as shell operation"
        );
    }

    #[test]
    fn test_extract_operations_shell_parens_detected() {
        let text = "Use shell() to hollow the body.";
        let ops = extract_operations(text);
        assert!(
            ops.contains(&"shell".to_string()),
            "shell() with parens should be detected"
        );

        let text2 = "Call shell (faces) to remove material.";
        let ops2 = extract_operations(text2);
        assert!(
            ops2.contains(&"shell".to_string()),
            "shell (with space before paren) should be detected"
        );
    }

    #[test]
    fn test_extract_operations_shell_action_verb_detected() {
        let text = "Apply shell to create a hollow part.";
        let ops = extract_operations(text);
        assert!(
            ops.contains(&"shell".to_string()),
            "'apply shell' should be detected"
        );

        let text2 = "Then shell the body to 2mm walls.";
        let ops2 = extract_operations(text2);
        assert!(
            ops2.contains(&"shell".to_string()),
            "'then shell' should be detected"
        );
    }

    #[test]
    fn test_validate_negative_dimension() {
        let text = "### Build Plan\n1. Create a box with -50mm height.";
        let v = validate_plan(text);
        assert!(v.risk_score >= 4);
        assert!(v.warnings.iter().any(|w| w.contains("negative dimension")));
    }

    #[test]
    fn test_validate_huge_dimension() {
        let text = "### Build Plan\n1. Create a beam 50000mm long, 10mm wide.";
        let v = validate_plan(text);
        assert!(v.risk_score >= 3);
        assert!(v.warnings.iter().any(|w| w.contains("exceeds 10 meters")));
    }

    #[test]
    fn test_validate_tiny_dimension() {
        let text = "### Build Plan\n1. Add a 0.005mm detail and a 50mm base.";
        let v = validate_plan(text);
        assert!(v.risk_score >= 3);
        assert!(v
            .warnings
            .iter()
            .any(|w| w.contains("below manufacturing precision")));
    }

    #[test]
    fn test_validate_missing_build_plan() {
        let text = "Just extrude a 50x30x20mm box.";
        let v = validate_plan(text);
        assert!(v.risk_score >= 2);
        assert!(v
            .warnings
            .iter()
            .any(|w| w.contains("missing a 'Build Plan'")));
    }

    #[test]
    fn test_validate_no_dimensions() {
        let text = "### Build Plan\n1. Make a box.\n2. Add a hole.";
        let v = validate_plan(text);
        assert!(v
            .warnings
            .iter()
            .any(|w| w.contains("no concrete dimensions")));
    }

    #[test]
    fn test_validate_fillet_on_small_feature() {
        let text =
            "### Build Plan\n1. Create a 15x15x10mm bracket.\n2. Apply fillet 8mm on edges.";
        let v = validate_plan(text);
        assert!(v
            .warnings
            .iter()
            .any(|w| w.contains("fillet radius") && w.contains("too large")));
    }

    #[test]
    fn test_validate_accumulates_to_rejection() {
        // shell + 5 booleans (+3 + +2) + negative dim (+4) = 9, clamped to 9 > 7
        let text = "### Build Plan\n\
            1. Cut slot.\n\
            2. Cut hole.\n\
            3. Cut pocket.\n\
            4. Union boss.\n\
            5. Fuse cap.\n\
            6. Shell the result to -10mm wall.";
        let v = validate_plan(text);
        assert!(!v.is_valid);
        assert!(v.risk_score > 7);
        assert!(v.rejected_reason.is_some());
    }

    #[test]
    fn test_validate_risk_score_clamped_to_10() {
        // Stack many rules: shell + 6 booleans (+3), booleans >= 5 (+2), negative (-50mm, +4),
        // huge (50000mm, +3), no build plan (+2), loft (+1), sweep (+1) = 16 → clamped to 10
        let text = "### Build Plan\n\
            1. Cut slot.\n\
            2. Cut vent.\n\
            3. Cut pocket.\n\
            4. Cut notch.\n\
            5. Union boss.\n\
            6. Fuse cap.\n\
            7. Shell the result.\n\
            8. Loft profiles for top blend.\n\
            9. Sweep guide rail profile.\n\
            10. Set dimensions to -50mm and 50000mm.";
        let v = validate_plan(text);
        assert_eq!(v.risk_score, 10);
        assert!(!v.is_valid);
    }

    // -----------------------------------------------------------------------
    // Feedback tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_build_rejection_feedback_includes_reason() {
        let v = PlanValidation {
            is_valid: false,
            risk_score: 8,
            warnings: vec!["shell() after 5 boolean operations is very likely to fail".to_string()],
            rejected_reason: Some(
                "shell() after 5 boolean operations is very likely to fail".to_string(),
            ),
            extracted_operations: vec!["shell".to_string()],
            extracted_dimensions: vec![100.0],
            plan_text: String::new(),
        };
        let fb = build_rejection_feedback(&v);
        assert!(fb.contains("risk score 8/10"));
        assert!(fb.contains("shell()"));
        assert!(fb.contains("Instructions for revision"));
    }

    #[test]
    fn test_build_rejection_feedback_lists_all_warnings() {
        let v = PlanValidation {
            is_valid: false,
            risk_score: 9,
            warnings: vec![
                "warning one".to_string(),
                "warning two".to_string(),
                "warning three".to_string(),
            ],
            rejected_reason: Some("warning one".to_string()),
            extracted_operations: vec![],
            extracted_dimensions: vec![],
            plan_text: String::new(),
        };
        let fb = build_rejection_feedback(&v);
        assert!(fb.contains("warning one"));
        assert!(fb.contains("warning two"));
        assert!(fb.contains("warning three"));
    }

    // -----------------------------------------------------------------------
    // Manufacturing constraints formatting tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_format_manufacturing_constraints_basic() {
        let yaml: serde_yaml::Value = serde_yaml::from_str(
            r#"
process: "3D Printing"
min_wall: 0.8
max_overhang: 45
"#,
        )
        .unwrap();
        let result = format_manufacturing_constraints(&yaml);
        assert!(result.contains("## Manufacturing Constraints"));
        assert!(result.contains("MUST respect"));
        assert!(result.contains("process"));
        assert!(result.contains("3D Printing"));
        assert!(result.contains("min_wall"));
        assert!(result.contains("0.8"));
        assert!(result.contains("max_overhang"));
        assert!(result.contains("45"));
    }

    #[test]
    fn test_format_manufacturing_constraints_nested() {
        let yaml: serde_yaml::Value = serde_yaml::from_str(
            r#"
process: "3D Printing (FDM/SLA)"
wall_thickness:
  minimum: 1.2
  recommended: 1.6
  rule: "All walls must be >= 1.2mm thick"
overhangs:
  max_angle: 45
  mitigation:
    - "Add chamfers instead of sharp overhangs"
    - "Use 45-degree angles where possible"
"#,
        )
        .unwrap();
        let result = format_manufacturing_constraints(&yaml);
        assert!(result.contains("## Manufacturing Constraints"));
        assert!(result.contains("wall_thickness"));
        assert!(result.contains("minimum"));
        assert!(result.contains("1.2"));
        assert!(result.contains("recommended"));
        assert!(result.contains("1.6"));
        assert!(result.contains("rule"));
        assert!(result.contains(">= 1.2mm"));
        assert!(result.contains("overhangs"));
        assert!(result.contains("max_angle"));
        assert!(result.contains("45"));
        assert!(result.contains("mitigation"));
        assert!(result.contains("Add chamfers"));
        assert!(result.contains("45-degree angles"));
    }

    #[test]
    fn test_format_manufacturing_constraints_empty_mapping() {
        let yaml: serde_yaml::Value = serde_yaml::from_str("{}").unwrap();
        let result = format_manufacturing_constraints(&yaml);
        assert!(result.contains("## Manufacturing Constraints"));
        assert!(result.contains("MUST respect"));
        // Header present but no bullet items beyond it
        let after_header = result.split("\n\n").last().unwrap_or("");
        assert!(after_header.trim().is_empty() || !after_header.contains("- **"));
    }

    // -----------------------------------------------------------------------
    // Dimension guidance formatting tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_format_dimension_guidance_basic() {
        let mut guidance = std::collections::HashMap::new();
        guidance.insert(
            "when_to_estimate".to_string(),
            vec![
                "For real-world objects, use typical dimensions".to_string(),
                "For abstract parts, ask the user".to_string(),
            ],
        );
        guidance.insert(
            "size_classes".to_string(),
            vec!["Tiny: < 20mm".to_string(), "Small: 20-60mm".to_string()],
        );
        let result = format_dimension_guidance(&guidance);
        assert!(result.contains("## Dimension Estimation Guidance"));
        assert!(result.contains("Follow these rules when choosing dimensions"));
        assert!(result.contains("real-world objects"));
        assert!(result.contains("Tiny: < 20mm"));
    }

    // -----------------------------------------------------------------------
    // Failure prevention formatting tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_format_failure_prevention_basic() {
        let mut prevention = std::collections::HashMap::new();
        prevention.insert(
            "self_diagnosis".to_string(),
            vec![
                "If fillet() fails, reduce the radius".to_string(),
                "If shell() raises an error, simplify first".to_string(),
            ],
        );
        prevention.insert(
            "preemptive_warnings".to_string(),
            vec!["If you are about to use shell() after booleans, STOP".to_string()],
        );
        let result = format_failure_prevention(&prevention);
        assert!(result.contains("## Failure Prevention Rules"));
        assert!(result.contains("Follow these rules when planning geometry"));
        assert!(result.contains("Self Diagnosis"));
        assert!(result.contains("Preemptive Warnings"));
        assert!(result.contains("fillet() fails"));
        assert!(result.contains("shell() after booleans"));
    }

    // -----------------------------------------------------------------------
    // New tests for validator improvements
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_dimensions_skips_offset_negative() {
        let text = "Offset by -6mm from the edge. Move -3mm along X.";
        let dims = extract_dimensions(text);
        // Both negatives have offset context words ("Offset", "Move") → skipped
        assert!(!dims.contains(&-6.0), "offset -6mm should be skipped");
        assert!(!dims.contains(&-3.0), "move -3mm should be skipped");
    }

    #[test]
    fn test_extract_chamfer_sizes() {
        let text = "Apply chamfer 1.5mm on bottom edges and chamfer(0.5) on top.";
        let sizes = extract_chamfer_sizes(text);
        assert!(sizes.contains(&1.5));
        assert!(sizes.contains(&0.5));
    }

    #[test]
    fn test_shell_with_fillet_not_flagged_as_thin_wall() {
        // 1mm is a fillet radius, not a wall thickness → Rule 13 should not fire
        let text = "### Object Analysis\nA hollow container.\n\n\
            ### CadQuery Approach\nShell and fillet.\n\n\
            ### Build Plan\n1. Create a 50x30x20mm box.\n\
            2. Shell with 3mm walls.\n3. Apply fillet 1mm on edges.";
        let v = validate_plan(text);
        assert!(
            !v.warnings
                .iter()
                .any(|w| w.contains("wall thickness") && w.contains("1mm")),
            "1mm fillet should not trigger thin wall warning: {:?}",
            v.warnings
        );
    }

    #[test]
    fn test_boolean_counting_ignores_prose_sections() {
        // "cut the design into sections" in Object Analysis is prose, not a boolean op
        let text = "### Object Analysis\nWe need to cut the design into sections for analysis. \
            We combine ideas to fuse the concept.\n\n\
            ### CadQuery Approach\nUse extrude.\n\n\
            ### Build Plan\n1. Extrude a 50x30x20mm box.";
        let count = count_boolean_mentions_in_build_plan(text);
        // Build Plan has no boolean ops
        assert_eq!(count, 0, "prose 'cut' and 'fuse' should not be counted");
    }

    #[test]
    fn test_extract_build_plan_steps_text_numbered_only() {
        let text = "### Build Plan\nOverview line.\n1. Extrude base.\n2. Cut slot.\nNotes.";
        let steps = extract_build_plan_steps_text(text).unwrap();
        assert!(steps.contains("1. Extrude base."));
        assert!(steps.contains("2. Cut slot."));
        assert!(!steps.contains("Overview line."));
        assert!(!steps.contains("Notes."));
    }

    #[test]
    fn test_validate_ignores_reasoning_outside_numbered_steps() {
        let text = "### Object Analysis\nbooleans booleans booleans cut union fuse intersect.\n\n\
            ### CadQuery Approach\nThis paragraph repeats cut/union/fuse many times cut cut cut.\n\n\
            ### Build Plan\n1. Extrude a 42x28x5mm rounded rectangle.";
        let v = validate_plan(text);
        assert!(
            !v.warnings.iter().any(|w| w.contains("boolean operations")),
            "should not warn on booleans outside numbered build steps: {:?}",
            v.warnings
        );
    }

    #[test]
    fn test_validate_requires_numbered_steps_when_build_plan_exists() {
        let text = "### Object Analysis\nA box.\n\n\
            ### CadQuery Approach\nUse extrude.\n\n\
            ### Build Plan\nCreate a 50x30x20mm box without numbering.";
        let v = validate_plan(text);
        assert!(v
            .warnings
            .iter()
            .any(|w| w.contains("must include numbered steps")));
    }

    #[test]
    fn test_sanitize_plan_text_strips_preamble() {
        let text = "Thinking aloud...\nMore analysis...\n\n### Object Analysis\nA part.";
        let sanitized = sanitize_plan_text(text);
        assert!(sanitized.starts_with("### Object Analysis"));
        assert!(!sanitized.contains("Thinking aloud"));
    }

    #[test]
    fn test_normalize_section_labels_from_emphasis_style() {
        let text = "Load template...\n\n\
            *Object Analysis*:\nA wearable housing.\n\n\
            *CadQuery Approach*:\nUse robust primitives.\n\n\
            *Build Plan Structure*:\n1. Extrude 42x28x5mm base.\n2. Add dome.\n\n\
            *Approximation Notes*:\nSimplify soft transitions.";
        let sanitized = sanitize_plan_text(text);
        assert!(sanitized.contains("### Object Analysis"));
        assert!(sanitized.contains("### CadQuery Approach"));
        assert!(sanitized.contains("### Build Plan"));
        assert!(sanitized.contains("### Approximation Notes"));
        assert!(sanitized.contains("1. Extrude 42x28x5mm base."));
        assert!(!sanitized.contains("Load template"));
    }

    #[test]
    fn test_extract_section_basic() {
        let text = "### Object Analysis\nA box shape.\n\n\
            ### Build Plan\n1. Extrude a box.\n2. Add fillets.\n\n\
            ### Approximation Notes\nNone.";
        let section = extract_section(text, "Build Plan");
        assert!(section.is_some());
        let body = section.unwrap();
        assert!(body.contains("Extrude a box"));
        assert!(body.contains("Add fillets"));
        assert!(
            !body.contains("box shape"),
            "should not include Object Analysis"
        );
        assert!(
            !body.contains("Approximation"),
            "should not include next section"
        );
    }

    #[test]
    fn test_operations_scoped_to_build_plan() {
        // "loft" and "sweep" only appear in CadQuery Approach (advisory), not Build Plan
        let text = "### Object Analysis\nA simple bracket.\n\n\
            ### CadQuery Approach\nCould use loft or sweep for organic shapes.\n\n\
            ### Build Plan\n1. Extrude a 50x30x10mm box.\n\
            2. Cut a 20mm hole through the center.";
        let v = validate_plan(text);
        assert!(
            !v.extracted_operations.contains(&"loft".to_string()),
            "loft from CadQuery Approach should not be extracted"
        );
        assert!(
            !v.extracted_operations.contains(&"sweep".to_string()),
            "sweep from CadQuery Approach should not be extracted"
        );
        assert!(v.extracted_operations.contains(&"extrude".to_string()));
        assert!(v.extracted_operations.contains(&"cut".to_string()));
    }

    #[test]
    fn test_structural_rules_15_16() {
        // Plan missing Object Analysis and CadQuery Approach → +1 each
        let text = "### Build Plan\n1. Extrude a 50x30x20mm box.";
        let v = validate_plan(text);
        assert!(v.warnings.iter().any(|w| w.contains("Object Analysis")));
        assert!(v.warnings.iter().any(|w| w.contains("CadQuery Approach")));
    }
}
