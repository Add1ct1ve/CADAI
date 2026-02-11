use crate::ai::message::ChatMessage;
use crate::ai::provider::{AiProvider, TokenUsage};
use crate::error::AppError;

const REVIEW_SYSTEM_PROMPT: &str = r#"You are a CadQuery code reviewer. Your job is to verify that generated CadQuery code correctly implements what the user requested.

Review the code against this checklist:
1. Are ALL requested features present? (e.g. if user asked for "two slots", are there two slots?)
2. Are dimensions correct and reasonable?
3. Do boolean cuts actually intersect the target geometry? (cuts positioned outside the body do nothing)
4. Are face selectors correct? ('>X' vs '<X', '>Z' vs '<Z', etc.)
5. Are operations applied to the right faces/edges?
6. Is the final result assigned to 'result'?
7. Does the code import cadquery as cq?

## CadQuery-Specific Pitfall Checks
8. Fillet radius must be SMALLER than the shortest edge it touches — if fillet(r) is called after a boolean, ensure r won't exceed any newly-created short edges
9. Shell fails on bodies with very thin features or complex topology — if shell() is used after many booleans, flag as risky
10. Revolve profiles must be entirely on ONE side of the rotation axis — check that no profile point crosses the axis
11. Sweep paths must produce a proper Wire object — ensure .wire() is called or the path is built with .lineTo()/.arc() chains
12. Boolean operations (union/cut/intersect) require OVERLAPPING geometry — bodies that don't touch produce no effect (silent failure)
13. .translate() takes a SINGLE tuple argument (x, y, z), NOT three separate arguments
14. .polarArray() must be called on a workplane context, not directly on a solid
15. Loft profiles should have compatible topology (similar number of edges) for clean results
16. Apply fillets and chamfers LAST, after all boolean operations are complete
17. If a complex operation (sweep, loft, shell on complex body) is likely to fail at runtime, suggest a simpler alternative using basic primitives + booleans
18. If code uses .edges().fillet() or .edges().chamfer() on geometry built with loft, shell, or multiple boolean operations, flag as HIGH RISK. Suggest wrapping in try/except: `try: result = body.edges().fillet(r)` / `except: result = body`

## Plan Compliance Checks (only when a Geometry Design Plan is provided)
19. Are ALL features listed in the Build Plan present in the code? (e.g., if plan says "add 4 mounting holes", there should be 4 holes in the code)
20. Do dimensions in the code match the plan? (e.g., if plan says "50x30x20mm box", the code should use 50, 30, 20 — not different values unless geometrically justified)
21. Does the code use the operations suggested in the plan? (e.g., if plan says "revolve a profile", the code should use revolve — not extrude — unless the alternative clearly achieves the same geometry)
22. Does the Build Plan step sequence roughly match the code's construction order? (e.g., if plan says "base shape, then holes, then fillets", the code should follow that order)

IMPORTANT for plan compliance:
- These checks are SOFT — the code may achieve the same geometry through different operations. If the result looks correct, APPROVE.
- Do NOT reject code solely because it uses a different operation than the plan suggested (e.g., loft instead of revolve is fine if the shape is correct).
- DO flag cases where planned features are completely missing or dimensions are significantly wrong (>20% off).
- If no Geometry Design Plan is provided, skip items 19-22 entirely.

If the code is correct, respond with exactly:
APPROVED

If there are issues, respond with:
ISSUES:
- [list each issue]

FIXED CODE:
```python
[corrected code here]
```

IMPORTANT:
- Only flag real geometric/logic errors, not style preferences
- If you fix the code, the fix must be a complete, self-contained script
- Do not add features the user didn't ask for
- Do not change dimensions unless they are clearly wrong
- Preserve validated generation contract and multipart architecture:
  - Keep `assy = cq.Assembly()`, `assy.add(part_...)`, and `result = assy.toCompound()` when present
  - Do not drop part variables or silently collapse multipart into single-part code
  - Do not replace a robust operation path with a riskier one unless required to fix a real defect
- When in doubt, APPROVE the code"#;

#[derive(Debug, Clone)]
pub struct ReviewResult {
    pub was_modified: bool,
    pub code: String,
    pub explanation: String,
}

/// Build the user message for the review prompt, optionally including the design plan.
fn build_review_user_message(
    user_request: &str,
    generated_code: &str,
    design_plan: Option<&str>,
) -> String {
    let mut content = format!("## User's Request\n{}", user_request);
    if let Some(plan) = design_plan {
        content.push_str(&format!("\n\n## Geometry Design Plan\n{}", plan));
    }
    content.push_str(&format!(
        "\n\n## Generated Code\n```python\n{}\n```",
        generated_code
    ));
    content
}

/// Review generated CadQuery code against the user's original request.
/// Returns the original or corrected code with an explanation.
pub async fn review_code(
    provider: Box<dyn AiProvider>,
    user_request: &str,
    generated_code: &str,
    design_plan: Option<&str>,
) -> Result<(ReviewResult, Option<TokenUsage>), AppError> {
    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: REVIEW_SYSTEM_PROMPT.to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: build_review_user_message(user_request, generated_code, design_plan),
        },
    ];

    let (response, usage) = provider.complete(&messages, Some(2048)).await?;

    Ok((parse_review_response(&response, generated_code), usage))
}

/// Parse the reviewer's response into a ReviewResult.
/// Falls back to keeping the original code if parsing fails.
fn parse_review_response(response: &str, original_code: &str) -> ReviewResult {
    let trimmed = response.trim();

    // Check for APPROVED
    if trimmed.starts_with("APPROVED") {
        return ReviewResult {
            was_modified: false,
            code: original_code.to_string(),
            explanation: "Code approved by reviewer.".to_string(),
        };
    }

    // Try to extract ISSUES and FIXED CODE
    if trimmed.contains("ISSUES:") || trimmed.contains("FIXED CODE:") {
        // Extract the explanation (everything between ISSUES: and FIXED CODE:)
        let explanation = if let Some(issues_start) = trimmed.find("ISSUES:") {
            let after_issues = &trimmed[issues_start + 7..];
            if let Some(fixed_start) = after_issues.find("FIXED CODE:") {
                after_issues[..fixed_start].trim().to_string()
            } else {
                after_issues.trim().to_string()
            }
        } else {
            "Reviewer found issues.".to_string()
        };

        // Extract the fixed code
        if let Some(fixed_code) = crate::agent::extract::extract_code(trimmed) {
            if !fixed_code.is_empty() {
                return ReviewResult {
                    was_modified: true,
                    code: fixed_code,
                    explanation,
                };
            }
        }
    }

    // Fallback: can't parse the response, keep original code
    ReviewResult {
        was_modified: false,
        code: original_code.to_string(),
        explanation: "Review completed (no changes).".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Review prompt content ──────────────────────────────────────────

    #[test]
    fn test_review_prompt_has_original_checklist() {
        assert!(REVIEW_SYSTEM_PROMPT.contains("ALL requested features"));
        assert!(REVIEW_SYSTEM_PROMPT.contains("dimensions correct"));
        assert!(REVIEW_SYSTEM_PROMPT.contains("boolean cuts"));
        assert!(REVIEW_SYSTEM_PROMPT.contains("face selectors"));
        assert!(REVIEW_SYSTEM_PROMPT.contains("assigned to 'result'"));
        assert!(REVIEW_SYSTEM_PROMPT.contains("import cadquery as cq"));
    }

    #[test]
    fn test_review_prompt_has_pitfall_checks() {
        assert!(
            REVIEW_SYSTEM_PROMPT.contains("Fillet radius must be SMALLER"),
            "should check fillet radius"
        );
        assert!(
            REVIEW_SYSTEM_PROMPT.contains("Shell fails on bodies with very thin features"),
            "should check shell on thin features"
        );
        assert!(
            REVIEW_SYSTEM_PROMPT.contains("Revolve profiles must be entirely on ONE side"),
            "should check revolve axis"
        );
        assert!(
            REVIEW_SYSTEM_PROMPT.contains("Sweep paths must produce a proper Wire"),
            "should check sweep wire"
        );
        assert!(
            REVIEW_SYSTEM_PROMPT.contains("OVERLAPPING geometry"),
            "should check boolean overlap"
        );
        assert!(
            REVIEW_SYSTEM_PROMPT.contains(".translate() takes a SINGLE tuple"),
            "should check translate signature"
        );
        assert!(
            REVIEW_SYSTEM_PROMPT.contains(".polarArray() must be called on a workplane"),
            "should check polarArray context"
        );
        assert!(
            REVIEW_SYSTEM_PROMPT.contains("fillets and chamfers LAST"),
            "should check fillets-last rule"
        );
        assert!(
            REVIEW_SYSTEM_PROMPT.contains("suggest a simpler alternative"),
            "should suggest simplification"
        );
    }

    #[test]
    fn test_review_prompt_has_numbered_items_8_through_18() {
        // Verify the pitfall check items are numbered 8-18
        for i in 8..=18 {
            assert!(
                REVIEW_SYSTEM_PROMPT.contains(&format!("{}.", i)),
                "should contain item number {}",
                i
            );
        }
    }

    // ── parse_review_response ──────────────────────────────────────────

    #[test]
    fn test_parse_approved() {
        let result = parse_review_response("APPROVED", "original code");
        assert!(!result.was_modified);
        assert_eq!(result.code, "original code");
        assert!(result.explanation.contains("approved"));
    }

    #[test]
    fn test_parse_approved_with_extra_text() {
        let result = parse_review_response("APPROVED\nThe code looks good.", "original code");
        assert!(!result.was_modified);
        assert_eq!(result.code, "original code");
    }

    #[test]
    fn test_parse_issues_with_fixed_code() {
        let response = r#"ISSUES:
- Fillet radius too large for the short edges created by the boolean
- translate() called with three arguments instead of a tuple

FIXED CODE:
```python
import cadquery as cq
result = cq.Workplane("XY").box(10, 10, 10)
```"#;
        let result = parse_review_response(response, "old code");
        assert!(result.was_modified);
        assert!(result.code.contains("import cadquery as cq"));
        assert!(result.explanation.contains("Fillet radius"));
    }

    #[test]
    fn test_parse_fallback_on_garbage() {
        let result = parse_review_response("some random text", "original code");
        assert!(!result.was_modified);
        assert_eq!(result.code, "original code");
        assert!(result.explanation.contains("no changes"));
    }

    #[test]
    fn test_parse_issues_without_code_block() {
        let response = "ISSUES:\n- Something is wrong\n\nBut I didn't provide fixed code.";
        let result = parse_review_response(response, "original code");
        // No code block means fallback to original
        assert!(!result.was_modified);
        assert_eq!(result.code, "original code");
    }

    // ── Plan compliance prompt tests ──────────────────────────────────

    #[test]
    fn test_review_prompt_has_plan_compliance_section() {
        assert!(REVIEW_SYSTEM_PROMPT.contains("Plan Compliance Checks"));
        assert!(REVIEW_SYSTEM_PROMPT.contains("ALL features listed in the Build Plan"));
        assert!(REVIEW_SYSTEM_PROMPT.contains("dimensions in the code match the plan"));
        assert!(REVIEW_SYSTEM_PROMPT.contains("operations suggested in the plan"));
        assert!(REVIEW_SYSTEM_PROMPT.contains("Build Plan step sequence"));
    }

    #[test]
    fn test_review_prompt_has_soft_check_guidance() {
        assert!(REVIEW_SYSTEM_PROMPT.contains("SOFT"));
        assert!(REVIEW_SYSTEM_PROMPT.contains("Do NOT reject code solely"));
        assert!(REVIEW_SYSTEM_PROMPT.contains("skip items 19-22"));
    }

    #[test]
    fn test_review_prompt_has_numbered_items_19_through_22() {
        for i in 19..=22 {
            assert!(
                REVIEW_SYSTEM_PROMPT.contains(&format!("{}.", i)),
                "should contain item number {}",
                i
            );
        }
    }

    #[test]
    fn test_review_prompt_has_blanket_fillet_check() {
        assert!(
            REVIEW_SYSTEM_PROMPT.contains(".edges().fillet()"),
            "should mention .edges().fillet() pattern"
        );
        assert!(
            REVIEW_SYSTEM_PROMPT.contains("HIGH RISK"),
            "should flag blanket fillet as high risk"
        );
        assert!(
            REVIEW_SYSTEM_PROMPT.contains("try/except"),
            "should suggest try/except wrapping"
        );
    }

    // ── build_review_user_message tests ───────────────────────────────

    #[test]
    fn test_build_review_message_without_plan() {
        let msg = build_review_user_message("make a box", "code here", None);
        assert!(msg.contains("## User's Request\nmake a box"));
        assert!(msg.contains("## Generated Code"));
        assert!(!msg.contains("## Geometry Design Plan"));
    }

    #[test]
    fn test_build_review_message_with_plan() {
        let plan = "### Build Plan\n1. Extrude a 50x30x20mm box";
        let msg = build_review_user_message("make a bracket", "code here", Some(plan));
        assert!(msg.contains("## Geometry Design Plan"));
        assert!(msg.contains("50x30x20mm box"));
    }

    #[test]
    fn test_build_review_message_plan_before_code() {
        let msg = build_review_user_message("req", "code", Some("plan text"));
        let plan_pos = msg.find("## Geometry Design Plan").unwrap();
        let code_pos = msg.find("## Generated Code").unwrap();
        assert!(plan_pos < code_pos, "plan should appear before code");
    }
}
