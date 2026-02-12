use regex::Regex;
use serde::Serialize;

use crate::ai::message::ChatMessage;
use crate::ai::provider::{AiProvider, TokenUsage};
use crate::config::GenerationReliabilityProfile;
use crate::error::AppError;

/// Result of prompt triage before plan generation.
#[derive(Debug, Clone, Serialize)]
pub struct PromptClarityAnalysis {
    pub needs_clarification: bool,
    pub questions: Vec<String>,
    pub enriched_prompt: Option<String>,
}

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
    pub negated_operations: Vec<String>,
    pub extracted_dimensions: Vec<f64>,
    pub risk_signals: PlanRiskSignals,
    pub plan_text: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct PlanRiskSignals {
    pub fatal_combo: bool,
    pub negation_conflict: bool,
    pub repair_sensitive_ops: Vec<String>,
}

/// Fast prompt triage. Currently conservative: only flags empty prompts.
pub async fn analyze_prompt_clarity(
    _provider: Box<dyn AiProvider>,
    message: &str,
) -> Result<PromptClarityAnalysis, AppError> {
    if message.trim().is_empty() {
        return Ok(PromptClarityAnalysis {
            needs_clarification: true,
            questions: vec!["What should the object be?".to_string()],
            enriched_prompt: None,
        });
    }

    Ok(PromptClarityAnalysis {
        needs_clarification: false,
        questions: vec![],
        enriched_prompt: None,
    })
}

const GEOMETRY_ADVISOR_PROMPT: &str = r#"You are a CAD geometry planner. Your job is to analyze a user's request and produce a detailed geometric build plan BEFORE any code is written.

You must think carefully about what the object actually looks like and how to build it with CadQuery primitives. Do NOT write any code — describe the geometry.
Do not include hidden reasoning, self-critique, or internal deliberation in the output.
Output only the final plan sections listed below.
The first line of your response MUST be exactly: `### Object Analysis`
Do not output any preamble text (e.g., "Load template...", "Let me think...", "Key requirements:").
Wrap the entire response in `<PLAN>...</PLAN>` tags.

## Your Output Format

### Object Analysis
Describe what this object looks like in the real world. What are its key visual features? What are its proportions? What makes it recognizable?

### CadQuery Approach
Which CadQuery primitives and operations best approximate each feature? Be specific:
- Prefer robust primary path first: extrude + cut/union with explicit intermediate solids
- Use loft()/sweep()/revolve() only when clearly necessary for geometry
- For enclosure-like objects, avoid `shell()` on lofted bodies in first pass
- For optional fidelity upgrade, describe a fallback path separately

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
- Reliability-first default: prefer "outer solid + inner subtract" for enclosures over `shell()` on lofted geometry
- Defer fillets/chamfers to optional last-step polish with fallback behavior
- For enclosure-like objects, include BOTH:
  1) a primary robust build path
  2) a fallback build path if the primary fails
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
const NEGATION_PREFIXES: &[&str] = &[
    "avoid",
    "without",
    "do not",
    "don't",
    "not use",
    "never use",
    "forbid",
    "forbidden",
    "exclude",
    "no ",
];
const REPAIR_SENSITIVE_OPS: &[&str] = &["shell", "loft", "sweep", "fillet", "chamfer"];

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
    // Common CAD phrasing: "shell from bottom face", "shell with thickness 2mm", "shelling"
    if Regex::new(r"\bshell\s+(?:from|with)\b")
        .unwrap()
        .is_match(&lower)
        || Regex::new(r"\bshelling\b").unwrap().is_match(&lower)
    {
        return true;
    }
    false
}

fn has_negated_operation(text: &str, op: &str) -> bool {
    let lower = text.to_lowercase();
    for prefix in NEGATION_PREFIXES {
        let p = regex::escape(prefix);
        let o = regex::escape(op);
        let pat = format!(r"(?i)\b{}\s+{}\b", p, o);
        if Regex::new(&pat).unwrap().is_match(&lower) {
            return true;
        }
    }

    // Handle compact forms like "no shell", "no loft", and explicit "without using loft".
    let alt = format!(
        r"(?i)\b(?:no|without)\s+(?:using\s+)?{}\b",
        regex::escape(op)
    );
    Regex::new(&alt).unwrap().is_match(&lower)
}

fn has_positive_operation(text: &str, op: &str) -> bool {
    if op == "shell" {
        return has_shell_operation(text);
    }

    let lower = text.to_lowercase();
    let escaped = regex::escape(op);
    let mention_re = Regex::new(&format!(r"(?i)(?:\.{}\s*\(|\b{}\b)", escaped, escaped)).unwrap();
    let trailing_neg_re = Regex::new(
        r"(?i)(?:avoid|without(?:\s+using)?|do\s+not|don't|not\s+use|never\s+use|forbid|forbidden|exclude|no)\s*$",
    )
    .unwrap();

    for m in mention_re.find_iter(&lower) {
        let prefix_start = m.start().saturating_sub(48);
        let prefix = lower[prefix_start..m.start()].trim_end();
        if !trailing_neg_re.is_match(prefix) {
            return true;
        }
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

fn extract_positive_operations(plan_text: &str) -> Vec<String> {
    let mut ops = Vec::new();
    for &op in KNOWN_OPERATIONS {
        if has_positive_operation(plan_text, op) {
            ops.push(op.to_string());
        }
    }
    ops
}

fn extract_negated_operations(plan_text: &str) -> Vec<String> {
    let mut ops = Vec::new();
    for &op in KNOWN_OPERATIONS {
        if has_negated_operation(plan_text, op) {
            ops.push(op.to_string());
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
    let plan_text = extract_plan_block(plan_text).unwrap_or(plan_text);

    // First try to normalize loosely formatted section labels (e.g. "*Object Analysis*:")
    // into canonical markdown headings.
    let normalized = if let Some(n) = normalize_section_labels(plan_text) {
        n
    } else {
        let heading_re = Regex::new(
            r"(?im)^#{2,3}\s+(object analysis|housing analysis|cadquery approach|modeling approach|cad approach|build plan|build plan structure|build steps|implementation steps|approximation notes)\s*$",
        )
        .unwrap();
        if let Some(m) = heading_re.find(plan_text) {
            plan_text[m.start()..].trim().to_string()
        } else {
            plan_text.trim().to_string()
        }
    };

    // Canonicalize output to stable sections and concise numbered build steps.
    canonicalize_plan_sections(&normalized)
}

/// Extract `<PLAN>...</PLAN>` block if present.
fn extract_plan_block(plan_text: &str) -> Option<&str> {
    let re = Regex::new(r"(?is)<plan>\s*(.*?)\s*</plan>").unwrap();
    re.captures(plan_text)
        .and_then(|caps| caps.get(1).map(|m| m.as_str()))
}

/// Keep section bodies concise and remove common planning chatter.
fn clean_section_lines(body: &str) -> Vec<String> {
    let meta_re = Regex::new(
        r"(?i)^\s*(load template|the user wants|let me|key requirements|detailed planning|this is tricky|actually|wait[, ]|analysis[: ]|details[: ]?)",
    )
    .unwrap();
    let mut out = Vec::new();
    for raw in body.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || meta_re.is_match(trimmed) {
            continue;
        }
        let normalized = trimmed
            .trim_start_matches("* ")
            .trim_start_matches("- ")
            .trim()
            .to_string();
        if normalized.is_empty() {
            continue;
        }
        out.push(format!("- {}", normalized));
        if out.len() >= 12 {
            break;
        }
    }
    out
}

/// Extract numbered steps and compress overly verbose step text.
fn extract_numbered_steps(text: &str) -> Vec<String> {
    let re = Regex::new(r"(?m)^\s*\d+[\.\)]\s+(.+)$").unwrap();
    let mut steps = Vec::new();
    for cap in re.captures_iter(text) {
        let mut step = cap
            .get(1)
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();
        if step.is_empty() {
            continue;
        }
        step = Regex::new(r"(?i)^details:\s*")
            .unwrap()
            .replace(&step, "")
            .to_string();
        // Collapse repeated whitespace to keep plans readable.
        step = Regex::new(r"\s+")
            .unwrap()
            .replace_all(&step, " ")
            .to_string();
        if step.len() > 260 {
            let clipped = &step[..260];
            if let Some(idx) = clipped.rfind(". ") {
                step = clipped[..idx + 1].to_string();
            } else if let Some(idx) = clipped.rfind("; ") {
                step = clipped[..idx + 1].to_string();
            } else {
                step = clipped.to_string();
            }
        }
        let step = step.trim().trim_end_matches(':').trim().to_string();
        if !step.is_empty() {
            steps.push(step);
        }
        if steps.len() >= 20 {
            break;
        }
    }
    steps
}

/// Build a canonical markdown plan with stable section ordering.
fn canonicalize_plan_sections(plan_text: &str) -> String {
    let object_analysis = extract_section(plan_text, "Object Analysis")
        .map(|s| clean_section_lines(&s))
        .unwrap_or_default();
    let cadquery_approach = extract_section(plan_text, "CadQuery Approach")
        .map(|s| clean_section_lines(&s))
        .unwrap_or_default();
    let approximation_notes = extract_section(plan_text, "Approximation Notes")
        .map(|s| clean_section_lines(&s))
        .unwrap_or_default();

    let build_plan_body = extract_section(plan_text, "Build Plan").unwrap_or_default();
    let has_build_plan_section = has_section(plan_text, "Build Plan");
    let mut build_steps = extract_numbered_steps(&build_plan_body);
    if build_steps.is_empty() {
        build_steps = extract_numbered_steps(plan_text);
    }

    let mut out = String::new();
    if !object_analysis.is_empty() {
        out.push_str("### Object Analysis\n");
        out.push_str(&object_analysis.join("\n"));
        out.push_str("\n\n");
    }

    if !cadquery_approach.is_empty() {
        out.push_str("### CadQuery Approach\n");
        out.push_str(&cadquery_approach.join("\n"));
        out.push_str("\n\n");
    }

    if !build_steps.is_empty() {
        out.push_str("### Build Plan\n");
        for (i, step) in build_steps.iter().enumerate() {
            out.push_str(&format!("{}. {}\n", i + 1, step));
        }
        out.push('\n');
    } else if has_build_plan_section {
        let fallback_lines = clean_section_lines(&build_plan_body);
        out.push_str("### Build Plan\n");
        if !fallback_lines.is_empty() {
            out.push_str(&fallback_lines.join("\n"));
            out.push('\n');
        }
        out.push('\n');
    }

    if !approximation_notes.is_empty() {
        out.push_str("### Approximation Notes\n");
        out.push_str(&approximation_notes.join("\n"));
    }

    if out.trim().is_empty() {
        plan_text.trim().to_string()
    } else {
        out.trim().to_string()
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
    fn parse_section_label_line(line: &str) -> Option<(&'static str, String)> {
        let re = Regex::new(
            r"(?i)^\s*(?:[-*]\s+|\d+[\.\)]\s+)?[*_`#\s]*(object analysis|housing analysis|cadquery approach|modeling approach|cad approach|build plan(?: structure)?|build steps|implementation steps|approximation notes)[*_`#\s]*:?\s*(.*)$",
        )
        .unwrap();
        let caps = re.captures(line)?;
        let raw_label = caps.get(1)?.as_str().to_lowercase();
        let remainder = caps
            .get(2)
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();

        let label = if raw_label == "object analysis" || raw_label == "housing analysis" {
            "Object Analysis"
        } else if raw_label == "cadquery approach"
            || raw_label == "modeling approach"
            || raw_label == "cad approach"
        {
            "CadQuery Approach"
        } else if raw_label == "build plan"
            || raw_label == "build plan structure"
            || raw_label == "build steps"
            || raw_label == "implementation steps"
        {
            "Build Plan"
        } else if raw_label == "approximation notes" {
            "Approximation Notes"
        } else {
            return None;
        };

        Some((label, remainder))
    }

    let mut current: Option<&'static str> = None;
    let mut sections: std::collections::HashMap<&'static str, Vec<String>> =
        std::collections::HashMap::new();
    let mut seen_any = false;

    for raw_line in plan_text.lines() {
        if let Some((lbl, remainder)) = parse_section_label_line(raw_line) {
            current = Some(lbl);
            seen_any = true;
            sections.entry(lbl).or_default();
            if !remainder.is_empty() {
                sections.entry(lbl).or_default().push(remainder);
            }
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
    let aliases: Vec<String> = match heading_lower.as_str() {
        "object analysis" => vec![
            "object analysis".to_string(),
            "housing analysis".to_string(),
        ],
        "cadquery approach" => vec![
            "cadquery approach".to_string(),
            "modeling approach".to_string(),
            "cad approach".to_string(),
        ],
        "build plan" => vec![
            "build plan".to_string(),
            "build plan structure".to_string(),
            "build steps".to_string(),
            "implementation steps".to_string(),
        ],
        _ => vec![heading_lower.clone()],
    };

    aliases.iter().any(|alias| {
        lower.contains(&format!("### {}", alias))
            || lower.contains(&format!("## {}", alias))
            || lower
                .lines()
                .any(|line| line.trim().to_lowercase() == *alias)
    })
}

/// Public wrapper for `extract_operations` — used by `iterative.rs` to detect risky ops.
pub fn extract_operations_from_text(text: &str) -> Vec<String> {
    extract_positive_operations(text)
}

// ---------------------------------------------------------------------------
// Plan validation
// ---------------------------------------------------------------------------

/// Validate a design plan deterministically (no AI calls).
///
/// Extracts operations and dimensions, calculates a risk score (0-10),
/// and rejects plans with a score > 7.
pub fn validate_plan_with_profile(
    plan_text: &str,
    profile: &GenerationReliabilityProfile,
) -> PlanValidation {
    let sanitized = sanitize_plan_text(plan_text);
    let plan_text = sanitized.as_str();

    // Scope operation/risk extraction to numbered Build Plan steps only.
    let build_plan_steps_text = extract_build_plan_steps_text(plan_text);
    let step_positive_operations = build_plan_steps_text
        .as_ref()
        .map(|s| extract_positive_operations(s))
        .unwrap_or_default();
    let step_negated_operations = build_plan_steps_text
        .as_ref()
        .map(|s| extract_negated_operations(s))
        .unwrap_or_default();

    // Scope dimensional risk checks to numbered Build Plan steps only.
    // This avoids false negatives/positives from prose or leaked "thinking" text.
    let step_dimensions = build_plan_steps_text
        .as_ref()
        .map(|s| extract_dimensions(s))
        .unwrap_or_default();
    // Presence checks aggregate Build Plan + full-plan mentions to avoid
    // underestimating risk when operations are described outside numbered steps.
    let full_positive_operations = extract_positive_operations(plan_text);
    let full_negated_operations = extract_negated_operations(plan_text);
    let full_dimensions = extract_dimensions(plan_text);
    let mut operations_for_presence = step_positive_operations.clone();
    for op in &full_positive_operations {
        if !operations_for_presence.contains(op) {
            operations_for_presence.push(op.clone());
        }
    }
    let mut negated_operations = step_negated_operations.clone();
    for op in &full_negated_operations {
        if !negated_operations.contains(op) {
            negated_operations.push(op.clone());
        }
    }
    let mut dimensions_for_presence = step_dimensions.clone();
    for d in &full_dimensions {
        if !dimensions_for_presence
            .iter()
            .any(|v| (v - d).abs() < 0.0001)
        {
            dimensions_for_presence.push(*d);
        }
    }

    let fillet_radii = extract_fillet_radii(plan_text);
    let chamfer_sizes = extract_chamfer_sizes(plan_text);
    // Scope boolean counting to numbered Build Plan steps only
    let boolean_count = build_plan_steps_text
        .as_ref()
        .map(|s| count_boolean_mentions(s))
        .unwrap_or(0);

    let has_shell = operations_for_presence.contains(&"shell".to_string());
    let has_loft = operations_for_presence.contains(&"loft".to_string());
    let has_sweep = operations_for_presence.contains(&"sweep".to_string());
    let has_revolve = operations_for_presence.contains(&"revolve".to_string());
    let has_chamfer = operations_for_presence.contains(&"chamfer".to_string());
    let has_fillet = operations_for_presence.contains(&"fillet".to_string());

    let mut negation_conflicts = Vec::new();
    for op in &operations_for_presence {
        if negated_operations.contains(op) {
            negation_conflicts.push(op.clone());
        }
    }
    let negation_conflict = !negation_conflicts.is_empty();

    let min_positive_dimension = dimensions_for_presence
        .iter()
        .copied()
        .filter(|&d| d > 0.0)
        .reduce(f64::min);

    let mut risk: u32 = 0;
    let mut warnings: Vec<String> = Vec::new();

    if negation_conflict {
        risk += 1;
        warnings.push(format!(
            "ambiguous operation intent: both positive and negated mentions for {}",
            negation_conflicts.join(", ")
        ));
    }

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

    // Rule 4b: loft + shell is a fatal reliability combo on first pass.
    let fatal_loft_shell = has_loft && has_shell;
    if fatal_loft_shell {
        risk += 3;
        warnings.push("loft() + shell() is a known reliability-killer combination".to_string());
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
    for &d in &dimensions_for_presence {
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
    for &d in &dimensions_for_presence {
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
    for &d in &dimensions_for_presence {
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
        warnings
            .push("Build Plan section must include numbered steps (1., 2., 3., ...)".to_string());
    }

    // Rule 11: no dimensions
    if dimensions_for_presence.is_empty() {
        risk += 2;
        warnings.push("no concrete dimensions found in plan".to_string());
    }

    // Rule 12: no operations
    if operations_for_presence.is_empty() {
        risk += 1;
        warnings.push("no CadQuery operations mentioned in plan".to_string());
    }

    // Rule 13: shell with thin walls (skip dimensions that are fillet radii or chamfer sizes)
    if has_shell {
        for &d in &dimensions_for_presence {
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

    // Rule 14b: shell + fillet is fragile for enclosure flows.
    let fatal_shell_internal_fillet = has_shell && has_fillet;
    if fatal_shell_internal_fillet {
        risk += 2;
        warnings
            .push("shell() combined with internal fillets is high risk in OpenCascade".to_string());
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

    let has_required_structure = has_section(plan_text, "Object Analysis")
        && has_section(plan_text, "CadQuery Approach")
        && has_section(plan_text, "Build Plan")
        && build_plan_steps_text.is_some();
    let risk_threshold = match profile {
        GenerationReliabilityProfile::ReliabilityFirst => 5,
        GenerationReliabilityProfile::Balanced => 7,
        GenerationReliabilityProfile::FidelityFirst => 8,
    };
    let has_fatal_reliability_combo =
        matches!(profile, GenerationReliabilityProfile::ReliabilityFirst)
            && (fatal_loft_shell || fatal_shell_internal_fillet);
    let repair_sensitive_ops = REPAIR_SENSITIVE_OPS
        .iter()
        .filter_map(|op| {
            if operations_for_presence.iter().any(|o| o == op) {
                Some((*op).to_string())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let risk_signals = PlanRiskSignals {
        fatal_combo: has_fatal_reliability_combo,
        negation_conflict,
        repair_sensitive_ops,
    };

    let is_valid = risk <= risk_threshold && has_required_structure && !has_fatal_reliability_combo;
    let rejected_reason = if !is_valid {
        if has_fatal_reliability_combo {
            Some("Reliability-first policy rejected fatal operation combo; re-plan with robust path.".to_string())
        } else {
            warnings.first().cloned()
        }
    } else {
        None
    };

    PlanValidation {
        is_valid,
        risk_score: risk,
        warnings,
        rejected_reason,
        extracted_operations: operations_for_presence,
        negated_operations,
        extracted_dimensions: dimensions_for_presence,
        risk_signals,
        plan_text: plan_text.to_string(),
    }
}

pub fn validate_plan(plan_text: &str) -> PlanValidation {
    validate_plan_with_profile(plan_text, &GenerationReliabilityProfile::Balanced)
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
         - For enclosure-type objects, include a primary robust path and a fallback path\n\
         - Use realistic, achievable dimensions\n\
         - Apply fillets and chamfers as the LAST operations\n\
         - Output EXACTLY these headings: `### Object Analysis`, `### CadQuery Approach`, `### Build Plan`, `### Approximation Notes`\n\
         - Include numbered steps in Build Plan (`1.`, `2.`, `3.`)\n\
         - Do NOT include any preamble text before `### Object Analysis`\n"
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
    fn test_extract_operations_shell_from_with_detected() {
        let text = "Shell from the bottom face to open the cavity.";
        let ops = extract_operations(text);
        assert!(
            ops.contains(&"shell".to_string()),
            "'shell from' should be detected"
        );

        let text2 = "Use shell with 1.8mm wall thickness.";
        let ops2 = extract_operations(text2);
        assert!(
            ops2.contains(&"shell".to_string()),
            "'shell with' should be detected"
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
        let text = "### Build Plan\n1. Create a 15x15x10mm bracket.\n2. Apply fillet 8mm on edges.";
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
            negated_operations: vec![],
            extracted_dimensions: vec![100.0],
            risk_signals: PlanRiskSignals::default(),
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
            negated_operations: vec![],
            extracted_dimensions: vec![],
            risk_signals: PlanRiskSignals::default(),
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
    fn test_normalize_section_labels_from_numbered_list_style() {
        let text = "Key aspects:\n\
            1. *Object Analysis*:\n\
            - Wrist-worn tracker housing.\n\
            2. *CadQuery Approach*:\n\
            - Use extrude, shell, cut, union.\n\
            3. *Build Plan*:\n\
            1. Extrude 42x28x5mm base.\n\
            2. Cut 20x2.5mm slots.\n\
            4. *Approximation Notes*:\n\
            - Dome approximated by cylindrical segment.";

        let sanitized = sanitize_plan_text(text);
        assert!(sanitized.contains("### Object Analysis"));
        assert!(sanitized.contains("### CadQuery Approach"));
        assert!(sanitized.contains("### Build Plan"));
        assert!(sanitized.contains("### Approximation Notes"));
        assert!(sanitized.contains("1. Extrude 42x28x5mm base."));
        assert!(sanitized.contains("2. Cut 20x2.5mm slots."));
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
    fn test_operations_include_presence_from_full_plan() {
        // "loft" and "sweep" appear in CadQuery Approach and are now retained
        // for reliability gating even if Build Plan uses simpler wording.
        let text = "### Object Analysis\nA simple bracket.\n\n\
            ### CadQuery Approach\nCould use loft or sweep for organic shapes.\n\n\
            ### Build Plan\n1. Extrude a 50x30x10mm box.\n\
            2. Cut a 20mm hole through the center.";
        let v = validate_plan(text);
        assert!(
            v.extracted_operations.contains(&"loft".to_string()),
            "loft from CadQuery Approach should be retained for reliability scoring"
        );
        assert!(
            v.extracted_operations.contains(&"sweep".to_string()),
            "sweep from CadQuery Approach should be retained for reliability scoring"
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

    #[test]
    fn test_reliability_first_rejects_loft_shell_combo() {
        let text = r#"### Object Analysis
- Enclosure.
### CadQuery Approach
- loft + shell.
### Build Plan
1. Loft a rounded enclosure profile.
2. Shell the body to create internal cavity.
### Approximation Notes
- None."#;

        let v = validate_plan_with_profile(text, &GenerationReliabilityProfile::ReliabilityFirst);
        assert!(!v.is_valid);
        assert!(v.risk_score >= 5);
        assert!(v.warnings.iter().any(|w| w.contains("loft() + shell()")));
    }

    #[test]
    fn test_reliability_first_rejects_when_combo_only_in_approach() {
        let text = r#"### Object Analysis
- Enclosure.
### CadQuery Approach
- Use loft() for dome and shell from the bottom face for cavity.
### Build Plan
1. Extrude base.
2. Cut openings.
### Approximation Notes
- None."#;

        let v = validate_plan_with_profile(text, &GenerationReliabilityProfile::ReliabilityFirst);
        assert!(!v.is_valid);
        assert!(
            v.extracted_operations.contains(&"loft".to_string())
                && v.extracted_operations.contains(&"shell".to_string())
        );
    }

    #[test]
    fn test_negated_operations_do_not_count_as_positive() {
        let text = r#"### Object Analysis
- Enclosure.
### CadQuery Approach
- Avoid shell and avoid loft for first pass.
### Build Plan
1. Extrude a 42x28x5mm base.
2. Cut a 20x2.5mm slot.
### Approximation Notes
- None."#;
        let v = validate_plan_with_profile(text, &GenerationReliabilityProfile::ReliabilityFirst);
        assert!(!v.extracted_operations.contains(&"shell".to_string()));
        assert!(!v.extracted_operations.contains(&"loft".to_string()));
        assert!(v.negated_operations.contains(&"shell".to_string()));
        assert!(v.negated_operations.contains(&"loft".to_string()));
        assert!(!v.risk_signals.fatal_combo);
    }

    #[test]
    fn test_positive_shell_still_detected() {
        let text = r#"### Object Analysis
- Enclosure.
### CadQuery Approach
- Robust enclosure path.
### Build Plan
1. Extrude a 42x28x5mm base.
2. Shell from bottom face with 1.8mm wall.
### Approximation Notes
- None."#;
        let v = validate_plan(text);
        assert!(v.extracted_operations.contains(&"shell".to_string()));
    }

    #[test]
    fn test_negation_conflict_emits_warning_and_signal() {
        let text = r#"### Object Analysis
- Enclosure.
### CadQuery Approach
- Avoid shell for first pass.
### Build Plan
1. Loft rounded outer body.
2. Shell from bottom face.
### Approximation Notes
- None."#;
        let v = validate_plan_with_profile(text, &GenerationReliabilityProfile::ReliabilityFirst);
        assert!(v.risk_signals.negation_conflict);
        assert!(v
            .warnings
            .iter()
            .any(|w| w.contains("ambiguous operation intent")));
    }

    #[test]
    fn test_balanced_profile_can_accept_simple_plan() {
        let text = r#"### Object Analysis
- Simple bracket.
### CadQuery Approach
- extrude + cut.
### Build Plan
1. Extrude a 50x30x10mm base.
2. Cut a 10mm hole through center.
### Approximation Notes
- None."#;

        let v_balanced = validate_plan_with_profile(text, &GenerationReliabilityProfile::Balanced);
        assert!(v_balanced.is_valid);
    }
}
