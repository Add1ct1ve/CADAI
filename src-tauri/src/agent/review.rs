use regex::Regex;

use crate::ai::message::ChatMessage;
use crate::ai::provider::AiProvider;
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
- When in doubt, APPROVE the code"#;

#[derive(Debug, Clone)]
pub struct ReviewResult {
    pub was_modified: bool,
    pub code: String,
    pub explanation: String,
}

/// Review generated CadQuery code against the user's original request.
/// Returns the original or corrected code with an explanation.
pub async fn review_code(
    provider: Box<dyn AiProvider>,
    user_request: &str,
    generated_code: &str,
) -> Result<ReviewResult, AppError> {
    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: REVIEW_SYSTEM_PROMPT.to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: format!(
                "## User's Request\n{}\n\n## Generated Code\n```python\n{}\n```",
                user_request, generated_code
            ),
        },
    ];

    let response = provider.complete(&messages, Some(2048)).await?;

    Ok(parse_review_response(&response, generated_code))
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
        let code_re = Regex::new(r"```python\s*\n([\s\S]*?)```").ok();
        if let Some(re) = code_re {
            if let Some(cap) = re.captures(trimmed) {
                let fixed_code = cap[1].trim().to_string();
                if !fixed_code.is_empty() {
                    return ReviewResult {
                        was_modified: true,
                        code: fixed_code,
                        explanation,
                    };
                }
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
