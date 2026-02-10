use base64::Engine;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::agent::design;
use crate::agent::executor::{self, ExecutionContext};
use crate::agent::extract;
use crate::agent::rules::AgentRules;
use crate::agent::validate;
use crate::ai::message::ChatMessage;
use crate::ai::provider::TokenUsage;
use crate::commands::chat::{build_retry_prompt, create_provider};
use crate::config::AppConfig;
use crate::error::AppError;

const MAX_STEP_RETRIES: u32 = 3;
const RISKY_OPS: &[&str] = &["shell", "loft", "sweep", "revolve"];

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// A single step parsed from the Build Plan.
#[derive(Debug, Clone, Serialize)]
pub struct BuildStep {
    pub index: usize,
    pub name: String,
    pub description: String,
    pub operations: Vec<String>,
}

/// Info about a step that was skipped (for retry).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedStep {
    pub step_index: usize,
    pub name: String,
    pub description: String,
    pub error: String,
}

/// Result of the full iterative build.
#[derive(Debug, Clone, Serialize)]
pub struct IterativeResult {
    pub final_code: String,
    pub stl_base64: Option<String>,
    pub success: bool,
    pub completed_steps: Vec<usize>,
    pub skipped_steps: Vec<SkippedStep>,
    pub total_usage: TokenUsage,
}

/// Progress events emitted during the iterative loop.
#[derive(Debug, Clone)]
pub enum IterativeEvent {
    Start {
        total_steps: usize,
        steps: Vec<BuildStep>,
    },
    StepStarted {
        step_index: usize,
        step_name: String,
        description: String,
    },
    StepComplete {
        step_index: usize,
        success: bool,
        stl_base64: Option<String>,
    },
    StepRetry {
        step_index: usize,
        attempt: u32,
        error: String,
    },
    StepSkipped {
        step_index: usize,
        name: String,
        error: String,
    },
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse "### Build Plan" section from the design plan text into BuildSteps.
///
/// Looks for numbered lines (1., 2., ...) inside the Build Plan section,
/// stopping at the next heading or end of text.
pub fn parse_build_steps(plan_text: &str) -> Vec<BuildStep> {
    let lines: Vec<&str> = plan_text.lines().collect();
    let mut in_build_plan = false;
    let mut steps = Vec::new();

    let heading_re = Regex::new(r"^#{1,4}\s+").unwrap();
    let build_plan_re = Regex::new(r"(?i)^#{1,4}\s+build\s+plan").unwrap();
    let step_re = Regex::new(r"^\s*(\d+)\.\s+(.+)").unwrap();

    for line in &lines {
        // Check if we enter the Build Plan section
        if build_plan_re.is_match(line) {
            in_build_plan = true;
            continue;
        }

        // If in Build Plan, stop at the next heading
        if in_build_plan && heading_re.is_match(line) && !build_plan_re.is_match(line) {
            break;
        }

        if in_build_plan {
            if let Some(cap) = step_re.captures(line) {
                let index: usize = cap[1].parse().unwrap_or(steps.len() + 1);
                let description = cap[2].trim().to_string();
                let name = generate_step_name(&description, index);
                let operations = design::extract_operations_from_text(&description);

                steps.push(BuildStep {
                    index,
                    name,
                    description,
                    operations,
                });
            }
        }
    }

    steps
}

/// Generate a snake_case name from a step description.
fn generate_step_name(description: &str, index: usize) -> String {
    // Take first few meaningful words and make a slug
    let lower = description.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .filter(|w| {
            !matches!(
                *w,
                "the" | "a" | "an" | "with" | "using" | "and" | "to" | "of" | "for" | "at" | "in"
                    | "on" | "by" | "from"
            )
        })
        .take(3)
        .collect();

    if words.is_empty() {
        format!("step_{}", index)
    } else {
        words.join("_")
    }
}

/// Should iterative mode be used?
///
/// Returns true if there are 4+ steps OR any step involves a risky operation.
pub fn should_use_iterative(steps: &[BuildStep]) -> bool {
    if steps.len() >= 4 {
        return true;
    }

    for step in steps {
        for op in &step.operations {
            if RISKY_OPS.contains(&op.as_str()) {
                return true;
            }
        }
    }

    false
}

// ---------------------------------------------------------------------------
// Step prompts
// ---------------------------------------------------------------------------

fn build_step_prompt(
    current_code: &str,
    step: &BuildStep,
    design_plan: &str,
    _user_request: &str,
) -> String {
    if current_code.is_empty() {
        format!(
            "Generate the initial CadQuery code for step {}: {}\n\n\
             Geometry Design Plan:\n{}\n\n\
             Rules:\n\
             - Start with `import cadquery as cq`\n\
             - The final variable MUST be called `result`\n\
             - Wrap your code in `<CODE>...</CODE>` tags\n\
             - Generate ONLY the code, no explanations",
            step.index, step.description, design_plan
        )
    } else {
        format!(
            "Here is the current working CadQuery code:\n\
             <CODE>\n{}\n</CODE>\n\n\
             Geometry Design Plan:\n{}\n\n\
             Add step {}: {}\n\n\
             Rules:\n\
             - Return the COMPLETE updated code\n\
             - Do NOT remove existing features — only ADD this step\n\
             - The final variable MUST still be called `result`\n\
             - Wrap your code in `<CODE>...</CODE>` tags\n\
             - Generate ONLY the code, no explanations",
            current_code, design_plan, step.index, step.description
        )
    }
}

fn build_step_retry_prompt(
    failed_code: &str,
    error_msg: &str,
    step: &BuildStep,
    design_plan: &str,
) -> String {
    // Use the structured error parsing for better retry guidance
    let structured_error = validate::parse_traceback(error_msg);
    let strategy = validate::get_retry_strategy(&structured_error, 1);

    // Look up anti-pattern if available
    let rules = AgentRules::from_preset(None).ok();
    let anti_pattern = rules.as_ref().and_then(|r| {
        r.anti_patterns.as_ref().and_then(|patterns| {
            strategy.matching_anti_pattern.as_ref().and_then(|title| {
                patterns.iter().find(|p| p.title == *title)
            })
        })
    });

    let base_retry =
        build_retry_prompt(failed_code, error_msg, &structured_error, &strategy, anti_pattern);

    format!(
        "{}\n\n\
         Context: This code is for step {} of an iterative build: {}\n\
         Design Plan:\n{}\n\n\
         Fix the error while keeping all existing features intact.\n\
         Wrap your code in `<CODE>...</CODE>` tags.",
        base_retry, step.index, step.description, design_plan
    )
}

// ---------------------------------------------------------------------------
// Iterative build loop
// ---------------------------------------------------------------------------

/// Run iterative build starting from empty code.
pub async fn run_iterative_build(
    steps: &[BuildStep],
    design_plan: &str,
    user_request: &str,
    system_prompt: &str,
    config: &AppConfig,
    ctx: &ExecutionContext,
    on_event: &(dyn Fn(IterativeEvent) + Send + Sync),
) -> Result<IterativeResult, AppError> {
    run_iterative_build_from(
        steps,
        "",
        design_plan,
        user_request,
        system_prompt,
        config,
        ctx,
        on_event,
    )
    .await
}

/// Run iterative build starting from existing code (for retrying skipped steps).
pub async fn run_iterative_build_from(
    steps: &[BuildStep],
    starting_code: &str,
    design_plan: &str,
    user_request: &str,
    system_prompt: &str,
    config: &AppConfig,
    ctx: &ExecutionContext,
    on_event: &(dyn Fn(IterativeEvent) + Send + Sync),
) -> Result<IterativeResult, AppError> {
    let mut current_code = starting_code.to_string();
    let mut current_stl: Option<String> = None;
    let mut completed_steps = Vec::new();
    let mut skipped_steps = Vec::new();
    let mut total_usage = TokenUsage::default();

    on_event(IterativeEvent::Start {
        total_steps: steps.len(),
        steps: steps.to_vec(),
    });

    for step in steps {
        on_event(IterativeEvent::StepStarted {
            step_index: step.index,
            step_name: step.name.clone(),
            description: step.description.clone(),
        });

        // Generate code for this step
        let step_prompt = build_step_prompt(&current_code, step, design_plan, user_request);
        let provider = create_provider(config)?;
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: step_prompt,
            },
        ];

        let (ai_response, usage) = provider.complete(&messages, None).await?;
        if let Some(ref u) = usage {
            total_usage.add(u);
        }

        let mut extracted = match extract::extract_code(&ai_response) {
            Some(code) => code,
            None => {
                // AI didn't produce extractable code — skip step
                on_event(IterativeEvent::StepSkipped {
                    step_index: step.index,
                    name: step.name.clone(),
                    error: "AI did not produce extractable code".to_string(),
                });
                skipped_steps.push(SkippedStep {
                    step_index: step.index,
                    name: step.name.clone(),
                    description: step.description.clone(),
                    error: "AI did not produce extractable code".to_string(),
                });
                continue;
            }
        };

        // Try executing with retries
        let mut step_succeeded = false;
        for attempt in 1..=MAX_STEP_RETRIES {
            match executor::execute_with_timeout(&extracted, &ctx.venv_dir, &ctx.runner_script)
                .await
            {
                Ok(exec_result) => {
                    // Success — update current state
                    current_code = extracted.clone();
                    let stl_b64 =
                        base64::engine::general_purpose::STANDARD.encode(&exec_result.stl_data);
                    current_stl = Some(stl_b64.clone());
                    completed_steps.push(step.index);
                    step_succeeded = true;

                    on_event(IterativeEvent::StepComplete {
                        step_index: step.index,
                        success: true,
                        stl_base64: Some(stl_b64),
                    });
                    break;
                }
                Err(error_msg) => {
                    if attempt < MAX_STEP_RETRIES {
                        on_event(IterativeEvent::StepRetry {
                            step_index: step.index,
                            attempt: attempt + 1,
                            error: error_msg.clone(),
                        });

                        // Ask AI for a fix
                        let retry_prompt =
                            build_step_retry_prompt(&extracted, &error_msg, step, design_plan);
                        let retry_provider = create_provider(config)?;
                        let retry_messages = vec![
                            ChatMessage {
                                role: "system".to_string(),
                                content: system_prompt.to_string(),
                            },
                            ChatMessage {
                                role: "user".to_string(),
                                content: retry_prompt,
                            },
                        ];

                        let (retry_response, retry_usage) =
                            retry_provider.complete(&retry_messages, None).await?;
                        if let Some(ref u) = retry_usage {
                            total_usage.add(u);
                        }

                        match extract::extract_code(&retry_response) {
                            Some(new_code) => {
                                extracted = new_code;
                            }
                            None => {
                                // AI retry didn't produce code — skip on next iteration
                            }
                        }
                    } else {
                        // All retries exhausted — skip this step
                        on_event(IterativeEvent::StepSkipped {
                            step_index: step.index,
                            name: step.name.clone(),
                            error: error_msg.clone(),
                        });
                        skipped_steps.push(SkippedStep {
                            step_index: step.index,
                            name: step.name.clone(),
                            description: step.description.clone(),
                            error: error_msg,
                        });
                    }
                }
            }
        }

        if !step_succeeded && !skipped_steps.iter().any(|s| s.step_index == step.index) {
            // Should not happen, but guard against edge cases
            skipped_steps.push(SkippedStep {
                step_index: step.index,
                name: step.name.clone(),
                description: step.description.clone(),
                error: "Step did not complete".to_string(),
            });
        }
    }

    let success = !completed_steps.is_empty();

    Ok(IterativeResult {
        final_code: current_code,
        stl_base64: current_stl,
        success,
        completed_steps,
        skipped_steps,
        total_usage,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_build_plan() {
        let plan = "\
### Object Analysis
A box with holes.

### Build Plan
1. Start with a 50x30x20mm box
2. Add 4 corner holes using cut cylinders
3. Apply 2mm fillets on top edges

### Approximation Notes
None.";
        let steps = parse_build_steps(plan);
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].index, 1);
        assert!(steps[0].description.contains("50x30x20mm box"));
        assert_eq!(steps[1].index, 2);
        assert!(steps[1].description.contains("holes"));
        assert_eq!(steps[2].index, 3);
        assert!(steps[2].description.contains("fillet"));
    }

    #[test]
    fn test_parse_empty_plan() {
        let plan = "Just some random text without a build plan section.";
        let steps = parse_build_steps(plan);
        assert!(steps.is_empty());
    }

    #[test]
    fn test_parse_plan_with_h2_heading() {
        let plan = "\
## Build Plan
1. Create base cylinder 50mm diameter
2. Shell to 2mm wall thickness";
        let steps = parse_build_steps(plan);
        assert_eq!(steps.len(), 2);
        assert!(steps[0].description.contains("cylinder"));
        assert!(steps[1].description.contains("shell") || steps[1].description.contains("Shell"));
    }

    #[test]
    fn test_parse_plan_stops_at_next_heading() {
        let plan = "\
### Build Plan
1. Create a box 50x30x20mm
2. Add a hole

### Approximation Notes
3. This should NOT be parsed as a step";
        let steps = parse_build_steps(plan);
        assert_eq!(steps.len(), 2);
    }

    #[test]
    fn test_iterative_triggered_by_step_count() {
        let steps: Vec<BuildStep> = (1..=5)
            .map(|i| BuildStep {
                index: i,
                name: format!("step_{}", i),
                description: format!("Extrude feature {}", i),
                operations: vec!["extrude".to_string()],
            })
            .collect();
        assert!(should_use_iterative(&steps));
    }

    #[test]
    fn test_iterative_not_triggered_few_simple_steps() {
        let steps = vec![
            BuildStep {
                index: 1,
                name: "create_box".to_string(),
                description: "Create a box".to_string(),
                operations: vec!["extrude".to_string()],
            },
            BuildStep {
                index: 2,
                name: "add_fillet".to_string(),
                description: "Add fillet".to_string(),
                operations: vec!["fillet".to_string()],
            },
        ];
        assert!(!should_use_iterative(&steps));
    }

    #[test]
    fn test_iterative_triggered_by_risky_op_shell() {
        let steps = vec![
            BuildStep {
                index: 1,
                name: "create_box".to_string(),
                description: "Create a box".to_string(),
                operations: vec!["extrude".to_string()],
            },
            BuildStep {
                index: 2,
                name: "shell_it".to_string(),
                description: "Shell to 2mm walls".to_string(),
                operations: vec!["shell".to_string()],
            },
        ];
        assert!(should_use_iterative(&steps));
    }

    #[test]
    fn test_iterative_triggered_by_loft() {
        let steps = vec![
            BuildStep {
                index: 1,
                name: "base_profile".to_string(),
                description: "Create base profile".to_string(),
                operations: vec![],
            },
            BuildStep {
                index: 2,
                name: "loft_profiles".to_string(),
                description: "Loft between profiles".to_string(),
                operations: vec!["loft".to_string()],
            },
        ];
        assert!(should_use_iterative(&steps));
    }

    #[test]
    fn test_step_name_from_description() {
        let name = generate_step_name("Start with a 50x30x20mm box", 1);
        assert!(!name.is_empty());
        assert!(!name.contains(' '));
        // Should filter out "start", "with", "a" and get something like "50x30x20mm_box"
    }

    #[test]
    fn test_step_name_empty_description() {
        let name = generate_step_name("", 3);
        assert_eq!(name, "step_3");
    }

    #[test]
    fn test_iterative_result_serialization() {
        let result = IterativeResult {
            final_code: "import cadquery as cq\nresult = cq.Workplane('XY').box(10,10,10)"
                .to_string(),
            stl_base64: Some("c3RsZGF0YQ==".to_string()),
            success: true,
            completed_steps: vec![1, 2, 3],
            skipped_steps: vec![],
            total_usage: TokenUsage::default(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"completed_steps\""));
    }

    #[test]
    fn test_skipped_step_serialization() {
        let skipped = SkippedStep {
            step_index: 3,
            name: "add_shell".to_string(),
            description: "Shell to 2mm walls".to_string(),
            error: "Shell operation failed".to_string(),
        };
        let json = serde_json::to_string(&skipped).unwrap();
        let deserialized: SkippedStep = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.step_index, 3);
        assert_eq!(deserialized.name, "add_shell");
    }

    #[test]
    fn test_build_step_serialization() {
        let step = BuildStep {
            index: 1,
            name: "create_box".to_string(),
            description: "Create a 50x30x20mm box".to_string(),
            operations: vec!["extrude".to_string()],
        };
        let json = serde_json::to_string(&step).unwrap();
        assert!(json.contains("\"index\":1"));
        assert!(json.contains("\"name\":\"create_box\""));
        assert!(json.contains("\"operations\""));
    }
}
