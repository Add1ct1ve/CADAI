use regex::Regex;
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};

// ---------------------------------------------------------------------------
// Structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ModificationIntent {
    pub is_modification: bool,
    pub intent_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    pub tag: String, // "equal", "insert", "delete"
    pub text: String,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default/template code markers — if the editor only has one of these,
/// there is no meaningful code to modify.
const DEFAULT_CODE_MARKERS: &[&str] = &[
    "result = Box(10, 10, 10)",
    "Box(10, 10, 10)",
    "# Create your 3D model here",
];

/// Regex patterns that indicate the user wants to *modify* existing code
/// rather than create something new from scratch.
const MODIFICATION_PATTERNS: &[&str] = &[
    r"(?i)\bmake\s+(?:it|the|this)\s+\w+(?:er|ier)\b",
    r"(?i)\b(?:add|insert|put|place)\s+(?:a|an|the|some)\b",
    r"(?i)\b(?:remove|delete|cut|take)\s+(?:out\s+)?(?:a|an|the|some)\b",
    r"(?i)\b(?:change|modify|adjust|update|alter|tweak|fix)\s",
    r"(?i)\b(?:increase|decrease|reduce|enlarge|shrink|scale|resize|double|halve|triple)\b",
    r"(?i)\b(?:move|shift|rotate|translate|offset|reposition)\b",
    r"(?i)\b(?:more|less|bigger|smaller|larger|shorter|longer|thicker|thinner|deeper|shallower)\b",
    r"(?i)\b(?:replace|swap)\b",
    r"(?i)\b(?:round|fillet|chamfer|bevel)\s+(?:the|all|every)\b",
    r"(?i)\b(?:hollow|shell)\s+(?:it|the|this)\b",
    r"(?i)\bto\s+\d+\s*(?:mm|cm|inch|in)\b",
    r"(?i)\bby\s+\d+\s*(?:mm|cm|inch|in)\b",
    r"(?i)\binstead\s+of\b",
];

/// System prompt addendum for modification mode.
pub const MODIFICATION_INSTRUCTIONS: &str = r#"
## MODIFICATION MODE
You are modifying existing Build123d code, NOT generating from scratch.

Critical rules:
1. Return the COMPLETE updated code (not just the changed parts)
2. Preserve existing variable names and code structure
3. Keep existing comments
4. Only change what the user asked for
5. The final result must still be assigned to `result`
6. Wrap in <CODE>...</CODE> tags
"#;

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// Two-signal detection: meaningful code must exist AND message must match
/// modification keywords. Returns false if code is None, empty, or just the
/// default template.
pub fn detect_modification_intent(
    user_message: &str,
    existing_code: Option<&str>,
) -> ModificationIntent {
    // Signal 1: Does meaningful code exist?
    let has_meaningful_code = match existing_code {
        None => false,
        Some(code) => {
            let trimmed = code.trim();
            if trimmed.is_empty() {
                return ModificationIntent {
                    is_modification: false,
                    intent_summary: None,
                };
            }
            // Check if the code is just a default template:
            // Any default marker present + short code = template, not meaningful
            let has_default_marker = DEFAULT_CODE_MARKERS
                .iter()
                .any(|marker| trimmed.contains(marker));
            let line_count = trimmed.lines().count();
            // Default template is ~4 lines; real code has more substance
            if has_default_marker && line_count <= 5 {
                false
            } else {
                line_count > 3
            }
        }
    };

    if !has_meaningful_code {
        return ModificationIntent {
            is_modification: false,
            intent_summary: None,
        };
    }

    // Signal 2: Does the message match modification patterns?
    let mut matched_pattern = None;
    for pattern in MODIFICATION_PATTERNS {
        if let Ok(re) = Regex::new(pattern) {
            if let Some(m) = re.find(user_message) {
                matched_pattern = Some(m.as_str().to_string());
                break;
            }
        }
    }

    match matched_pattern {
        Some(summary) => ModificationIntent {
            is_modification: true,
            intent_summary: Some(summary),
        },
        None => ModificationIntent {
            is_modification: false,
            intent_summary: None,
        },
    }
}

// ---------------------------------------------------------------------------
// Prompt building
// ---------------------------------------------------------------------------

/// Build the user message for modification mode.
/// Wraps the existing code and the modification request in a structured format.
pub fn build_modification_message(existing_code: &str, user_request: &str) -> String {
    format!(
        "## Existing Code\n```python\n{}\n```\n\n## Modification Request\n{}",
        existing_code, user_request
    )
}

// ---------------------------------------------------------------------------
// Diff computation
// ---------------------------------------------------------------------------

/// Compute a line-level diff between old and new code using the `similar` crate.
pub fn compute_diff(old_code: &str, new_code: &str) -> Vec<DiffLine> {
    let diff = TextDiff::from_lines(old_code, new_code);
    diff.iter_all_changes()
        .map(|change| {
            let tag = match change.tag() {
                ChangeTag::Equal => "equal",
                ChangeTag::Insert => "insert",
                ChangeTag::Delete => "delete",
            };
            DiffLine {
                tag: tag.to_string(),
                text: change.value().trim_end_matches('\n').to_string(),
            }
        })
        .collect()
}

/// Returns true if the diff contains any insert or delete lines.
pub fn diff_has_changes(diff: &[DiffLine]) -> bool {
    diff.iter().any(|line| line.tag != "equal")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const REAL_CODE: &str = r#"from build123d import *

# Create a box with rounded edges
result = fillet(Box(50, 30, 20).edges().filter_by(Axis.Z), radius=3)
"#;

    const DEFAULT_TEMPLATE: &str = r#"from build123d import *

# Create your 3D model here
result = Box(10, 10, 10)
"#;

    #[test]
    fn test_detect_modification_make_taller() {
        let intent = detect_modification_intent("make it taller", Some(REAL_CODE));
        assert!(intent.is_modification);
        assert!(intent.intent_summary.is_some());
    }

    #[test]
    fn test_detect_modification_add_hole() {
        let intent = detect_modification_intent("add a hole in the top", Some(REAL_CODE));
        assert!(intent.is_modification);
        assert!(intent.intent_summary.is_some());
    }

    #[test]
    fn test_detect_no_modification_create() {
        let intent = detect_modification_intent("create a cylinder", Some(REAL_CODE));
        assert!(!intent.is_modification);
    }

    #[test]
    fn test_detect_no_modification_default_code() {
        let intent = detect_modification_intent("make it taller", Some(DEFAULT_TEMPLATE));
        assert!(!intent.is_modification);
    }

    #[test]
    fn test_detect_no_modification_no_code() {
        let intent = detect_modification_intent("make it taller", None);
        assert!(!intent.is_modification);
    }

    #[test]
    fn test_compute_diff_identical() {
        let diff = compute_diff(REAL_CODE, REAL_CODE);
        assert!(!diff.is_empty());
        assert!(diff.iter().all(|line| line.tag == "equal"));
    }

    #[test]
    fn test_compute_diff_changes() {
        let new_code = REAL_CODE.replace("Box(50, 30, 20)", "Box(50, 30, 40)");
        let diff = compute_diff(REAL_CODE, &new_code);
        assert!(diff_has_changes(&diff));
        // Should have at least one insert and one delete
        assert!(diff.iter().any(|l| l.tag == "insert"));
        assert!(diff.iter().any(|l| l.tag == "delete"));
    }

    #[test]
    fn test_diff_has_changes_true() {
        let diff = vec![
            DiffLine {
                tag: "equal".to_string(),
                text: "line 1".to_string(),
            },
            DiffLine {
                tag: "insert".to_string(),
                text: "new line".to_string(),
            },
        ];
        assert!(diff_has_changes(&diff));
    }

    #[test]
    fn test_diff_has_changes_false() {
        let diff = vec![
            DiffLine {
                tag: "equal".to_string(),
                text: "line 1".to_string(),
            },
            DiffLine {
                tag: "equal".to_string(),
                text: "line 2".to_string(),
            },
        ];
        assert!(!diff_has_changes(&diff));
    }

    #[test]
    fn test_build_modification_message_format() {
        let msg = build_modification_message("code here", "make it bigger");
        assert!(msg.contains("## Existing Code"));
        assert!(msg.contains("```python"));
        assert!(msg.contains("code here"));
        assert!(msg.contains("## Modification Request"));
        assert!(msg.contains("make it bigger"));
    }

    #[test]
    fn test_modification_message_preserves_code() {
        let code = "result = fillet(Box(10, 10, 10).edges(), radius=2)";
        let msg = build_modification_message(code, "add a hole");
        assert!(msg.contains(code));
    }
}
