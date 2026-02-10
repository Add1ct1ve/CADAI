use regex::Regex;
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::State;
use tokio::sync::mpsc;

use crate::ai::message::ChatMessage;
use crate::ai::provider::StreamDelta;
use crate::agent::design;
use crate::agent::prompts;
use crate::agent::review;
use crate::error::AppError;
use crate::state::AppState;

use super::chat::create_provider;

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationPlan {
    pub mode: String,
    pub description: Option<String>,
    #[serde(default)]
    pub parts: Vec<PartSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartSpec {
    pub name: String,
    pub description: String,
    pub position: [f64; 3],
    #[serde(default)]
    pub constraints: Vec<String>,
}

/// Events streamed to the frontend over a Tauri Channel during parallel generation.
#[derive(Clone, Serialize)]
#[serde(tag = "kind")]
pub enum MultiPartEvent {
    /// Geometry design plan produced before code generation.
    DesignPlan {
        plan_text: String,
    },
    /// Result of deterministic plan validation.
    PlanValidation {
        risk_score: u32,
        warnings: Vec<String>,
        is_valid: bool,
        rejected_reason: Option<String>,
    },
    PlanStatus {
        message: String,
    },
    PlanResult {
        plan: GenerationPlan,
    },
    /// Streaming delta for a single-mode fallback (acts like StreamEvent).
    SingleDelta {
        delta: String,
        done: bool,
    },
    /// Full response for single-mode (carries the complete text).
    SingleDone {
        full_response: String,
    },
    PartDelta {
        part_index: usize,
        part_name: String,
        delta: String,
    },
    PartComplete {
        part_index: usize,
        part_name: String,
        success: bool,
        error: Option<String>,
    },
    AssemblyStatus {
        message: String,
    },
    FinalCode {
        code: String,
    },
    ReviewStatus {
        message: String,
    },
    ReviewComplete {
        was_modified: bool,
        explanation: String,
    },
    Done {
        success: bool,
        error: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Prompts
// ---------------------------------------------------------------------------

const PLANNER_SYSTEM_PROMPT: &str = r#"You are a CAD decomposition planner. Analyze the user's request (which includes a geometry design plan) and decide whether it should be built as a single part or decomposed into multiple parts.

Return ONLY valid JSON (no markdown fences, no extra text).

If the request is a single object (even a complex one like a helmet or handle), return:
{"mode": "single"}

If the request involves 2-4 clearly distinct SEPARABLE components that fit together (e.g. a bottle with a cap, a box with a lid, a phone with a case), return:
{
  "mode": "multi",
  "description": "Brief description of the overall assembly",
  "parts": [
    {
      "name": "snake_case_name",
      "description": "Detailed geometric description with ALL dimensions in mm. Include the specific CadQuery operations to use (loft, revolve, booleans). Reference the geometry design plan. This description must be fully self-contained.",
      "position": [x, y, z],
      "constraints": ["any constraints like 'inner diameter must match outer diameter of part X'"]
    }
  ]
}

## When to use multi mode
- The request describes 2-4 PHYSICALLY SEPARATE objects that assemble together
- Example: "bottle with screw-on cap" → multi (bottle body + cap)
- Example: "laptop stand with adjustable hinge" → multi (base + arm + platform)

## When to use single mode
- The request is ONE object, even if complex (helmet, phone case, vase, gear)
- Features like holes, slots, fillets are modifications of one body, NOT separate parts
- Complex shapes built from boolean operations on one body → single mode

## Part description requirements (multi mode only)
- Include specific CadQuery operations: "Use loft() between ellipses at heights 0, 80, 160mm"
- Include all dimensions in mm
- Include geometric detail from the design plan: profiles, cross-sections, radii
- Each part description must be self-contained (another AI must be able to build it without other context)

Rules:
- Part names must be valid Python identifiers (snake_case)
- Positions are in mm, relative to origin [0,0,0]
- Do NOT decompose decorative features, fillets, or chamfers into separate parts

Keep your response as short as possible. For single mode, return ONLY {"mode":"single"} with no other text."#;

fn build_part_prompt(system_prompt: &str, part: &PartSpec, design_context: &str) -> String {
    format!(
        "{}\n\n\
        ## Geometry Design Context\n{}\n\n\
        ## IMPORTANT: You are generating ONE SPECIFIC PART of a multi-part assembly.\n\n\
        Generate ONLY this part: **{}**\n\n\
        Description: {}\n\n\
        Constraints:\n{}\n\n\
        The final result variable MUST contain ONLY this single part.\n\
        Do NOT generate any other parts. Do NOT create an assembly.\n\
        Wrap your code in a ```python code block.",
        system_prompt,
        design_context,
        part.name,
        part.description,
        part.constraints
            .iter()
            .map(|c| format!("- {}", c))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

// ---------------------------------------------------------------------------
// Assembly
// ---------------------------------------------------------------------------

fn assemble_parts(parts: &[(String, String, [f64; 3])]) -> Result<String, String> {
    // parts: Vec<(name, code, position)>
    if parts.is_empty() {
        return Err("No parts to assemble".to_string());
    }

    let mut assembled = String::new();
    assembled.push_str("import cadquery as cq\n\n");

    // Process each part: strip duplicate imports and rename `result` → `part_{name}`
    let result_re = Regex::new(r"\bresult\b").unwrap();

    for (name, code, _pos) in parts {
        let var_name = format!("part_{}", name);

        // Strip import lines (we already have the import at the top)
        let cleaned: Vec<&str> = code
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.starts_with("import cadquery")
                    && !trimmed.starts_with("from cadquery")
            })
            .collect();

        // Rename `result` to `part_{name}`
        let renamed = result_re
            .replace_all(&cleaned.join("\n"), var_name.as_str())
            .to_string();

        assembled.push_str(&format!("# --- {} ---\n", name));
        assembled.push_str(&renamed);
        assembled.push_str("\n\n");
    }

    // Build the assembly
    assembled.push_str("# --- Assembly ---\n");
    assembled.push_str("assy = cq.Assembly()\n");

    for (name, _code, pos) in parts {
        let var_name = format!("part_{}", name);
        assembled.push_str(&format!(
            "assy.add({}, name=\"{}\", loc=cq.Location(({}, {}, {})))\n",
            var_name, name, pos[0], pos[1], pos[2],
        ));
    }

    assembled.push_str("result = assy.toCompound()\n");

    Ok(assembled)
}

// ---------------------------------------------------------------------------
// Command
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn generate_parallel(
    message: String,
    history: Vec<ChatMessage>,
    on_event: Channel<MultiPartEvent>,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let config = state.config.lock().unwrap().clone();
    let system_prompt =
        prompts::build_system_prompt_for_preset(config.agent_rules_preset.as_deref());
    let user_request = message.clone();

    // -----------------------------------------------------------------------
    // Phase 0: Geometry Design Plan (always runs)
    // -----------------------------------------------------------------------
    let _ = on_event.send(MultiPartEvent::PlanStatus {
        message: "Designing geometry...".to_string(),
    });

    let design_provider = create_provider(&config)?;
    let mut design_plan = design::plan_geometry(design_provider, &message).await?;

    // Validate the design plan (deterministic, no AI call).
    let validation = design::validate_plan(&design_plan.text);

    let _ = on_event.send(MultiPartEvent::PlanValidation {
        risk_score: validation.risk_score,
        warnings: validation.warnings.clone(),
        is_valid: validation.is_valid,
        rejected_reason: validation.rejected_reason.clone(),
    });

    // If plan is too risky, re-prompt once with feedback.
    if !validation.is_valid {
        let _ = on_event.send(MultiPartEvent::PlanStatus {
            message: format!(
                "Design plan too risky (score {}/10), re-planning...",
                validation.risk_score
            ),
        });

        let feedback = design::build_rejection_feedback(&validation);
        let retry_provider = create_provider(&config)?;
        design_plan = design::plan_geometry_with_feedback(
            retry_provider, &message, &feedback
        ).await?;

        // Validate retry but accept even if still risky (max 1 retry).
        let retry_validation = design::validate_plan(&design_plan.text);
        let _ = on_event.send(MultiPartEvent::PlanValidation {
            risk_score: retry_validation.risk_score,
            warnings: retry_validation.warnings.clone(),
            is_valid: retry_validation.is_valid,
            rejected_reason: retry_validation.rejected_reason.clone(),
        });
    }

    // Send the (possibly revised) design plan to the frontend.
    let _ = on_event.send(MultiPartEvent::DesignPlan {
        plan_text: design_plan.text.clone(),
    });

    // Build an enhanced message that includes the design plan as context
    // for all downstream code generation.
    let enhanced_message = format!(
        "## Geometry Design Plan\n{}\n\n## User Request\n{}",
        design_plan.text, message
    );

    // -----------------------------------------------------------------------
    // Phase 1: Plan (decomposition)
    // -----------------------------------------------------------------------
    let _ = on_event.send(MultiPartEvent::PlanStatus {
        message: "Analyzing request...".to_string(),
    });

    let planner = create_provider(&config)?;

    let planner_messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: PLANNER_SYSTEM_PROMPT.to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: enhanced_message.clone(),
        },
    ];

    let plan_json = planner.complete(&planner_messages, Some(1024)).await?;

    // Try to parse the plan; fall back to single mode on failure.
    let plan: GenerationPlan = parse_plan(&plan_json);

    let _ = on_event.send(MultiPartEvent::PlanResult {
        plan: plan.clone(),
    });

    // -----------------------------------------------------------------------
    // Single mode: fall through to normal streaming
    // -----------------------------------------------------------------------
    if plan.mode == "single" || plan.parts.is_empty() {
        let _ = on_event.send(MultiPartEvent::PlanStatus {
            message: "Generating code...".to_string(),
        });

        let provider = create_provider(&config)?;

        let mut messages_list = vec![ChatMessage {
            role: "system".to_string(),
            content: system_prompt,
        }];
        messages_list.extend(history);
        // Use the enhanced message that includes the geometry design plan
        messages_list.push(ChatMessage {
            role: "user".to_string(),
            content: enhanced_message.clone(),
        });

        // Stream via the same channel using SingleDelta events.
        let (tx, mut rx) = mpsc::channel::<StreamDelta>(100);
        let provider_handle =
            tokio::spawn(async move { provider.stream(&messages_list, tx).await });

        let mut full_response = String::new();
        while let Some(delta) = rx.recv().await {
            full_response.push_str(&delta.content);
            let _ = on_event.send(MultiPartEvent::SingleDelta {
                delta: delta.content,
                done: delta.done,
            });
        }

        match provider_handle.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e),
            Err(e) => {
                return Err(AppError::AiProviderError(format!(
                    "Provider task panicked: {}",
                    e
                )));
            }
        }

        let _ = on_event.send(MultiPartEvent::SingleDone {
            full_response: full_response.clone(),
        });

        // Review step: verify the generated code matches the request
        if config.enable_code_review {
            if let Some(code) = extract_code_from_response(&full_response) {
                let _ = on_event.send(MultiPartEvent::ReviewStatus {
                    message: "Reviewing generated code...".to_string(),
                });

                let review_provider = create_provider(&config)?;
                match review::review_code(review_provider, &user_request, &code).await {
                    Ok(result) => {
                        let _ = on_event.send(MultiPartEvent::ReviewComplete {
                            was_modified: result.was_modified,
                            explanation: result.explanation.clone(),
                        });
                        if result.was_modified {
                            let _ = on_event.send(MultiPartEvent::FinalCode {
                                code: result.code.clone(),
                            });
                            // Replace code in the response text
                            let updated_response = full_response.replace(
                                &code,
                                &result.code,
                            );
                            let _ = on_event.send(MultiPartEvent::Done {
                                success: true,
                                error: None,
                            });
                            return Ok(updated_response);
                        }
                    }
                    Err(e) => {
                        // Review failed, continue with original code
                        eprintln!("Code review failed: {}", e);
                    }
                }
            }
        }

        let _ = on_event.send(MultiPartEvent::Done {
            success: true,
            error: None,
        });

        return Ok(full_response);
    }

    // -----------------------------------------------------------------------
    // Phase 2: Parallel generation
    // -----------------------------------------------------------------------
    let _ = on_event.send(MultiPartEvent::PlanStatus {
        message: format!("Generating {} parts in parallel...", plan.parts.len()),
    });

    let mut handles = Vec::new();

    for (idx, part) in plan.parts.iter().enumerate() {
        let part_provider = create_provider(&config)?;
        let part_prompt = build_part_prompt(&system_prompt, part, &design_plan.text);
        let part_name = part.name.clone();
        let event_channel = on_event.clone();

        let part_messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: part_prompt,
            },
            ChatMessage {
                role: "user".to_string(),
                content: format!(
                    "Generate the CadQuery code for: {}",
                    part.description
                ),
            },
        ];

        let handle = tokio::spawn(async move {
            let (tx, mut rx) = mpsc::channel::<StreamDelta>(100);

            let stream_handle =
                tokio::spawn(async move { part_provider.stream(&part_messages, tx).await });

            let mut full_response = String::new();
            while let Some(delta) = rx.recv().await {
                full_response.push_str(&delta.content);
                let _ = event_channel.send(MultiPartEvent::PartDelta {
                    part_index: idx,
                    part_name: part_name.clone(),
                    delta: delta.content,
                });
            }

            let result = match stream_handle.await {
                Ok(Ok(())) => Ok(full_response),
                Ok(Err(e)) => Err(e.to_string()),
                Err(e) => Err(format!("Part task panicked: {}", e)),
            };

            (idx, result)
        });

        handles.push((idx, part.name.clone(), handle));
    }

    // Collect results
    let mut part_codes: Vec<Option<(String, String, [f64; 3])>> =
        vec![None; plan.parts.len()];
    let mut any_success = false;

    for (idx, name, handle) in handles {
        let position = plan.parts[idx].position;

        match handle.await {
            Ok((_, Ok(response))) => {
                // Extract python code from the response
                let code = extract_code_from_response(&response);
                match code {
                    Some(c) => {
                        part_codes[idx] = Some((name.clone(), c, position));
                        any_success = true;
                        let _ = on_event.send(MultiPartEvent::PartComplete {
                            part_index: idx,
                            part_name: name,
                            success: true,
                            error: None,
                        });
                    }
                    None => {
                        let _ = on_event.send(MultiPartEvent::PartComplete {
                            part_index: idx,
                            part_name: name,
                            success: false,
                            error: Some("No code block found in response".to_string()),
                        });
                    }
                }
            }
            Ok((_, Err(e))) => {
                let _ = on_event.send(MultiPartEvent::PartComplete {
                    part_index: idx,
                    part_name: name,
                    success: false,
                    error: Some(e),
                });
            }
            Err(e) => {
                let _ = on_event.send(MultiPartEvent::PartComplete {
                    part_index: idx,
                    part_name: name,
                    success: false,
                    error: Some(format!("Task join error: {}", e)),
                });
            }
        }
    }

    if !any_success {
        let _ = on_event.send(MultiPartEvent::Done {
            success: false,
            error: Some("All parts failed to generate".to_string()),
        });
        return Err(AppError::AiProviderError(
            "All parts failed to generate".to_string(),
        ));
    }

    // -----------------------------------------------------------------------
    // Phase 3: Assemble
    // -----------------------------------------------------------------------
    let _ = on_event.send(MultiPartEvent::AssemblyStatus {
        message: "Assembling parts...".to_string(),
    });

    let successful_parts: Vec<(String, String, [f64; 3])> = part_codes
        .into_iter()
        .flatten()
        .collect();

    match assemble_parts(&successful_parts) {
        Ok(code) => {
            // Review assembled code before finalizing
            let final_code = if config.enable_code_review {
                let _ = on_event.send(MultiPartEvent::ReviewStatus {
                    message: "Reviewing assembled code...".to_string(),
                });
                let review_provider = create_provider(&config)?;
                match review::review_code(review_provider, &user_request, &code).await {
                    Ok(result) => {
                        let _ = on_event.send(MultiPartEvent::ReviewComplete {
                            was_modified: result.was_modified,
                            explanation: result.explanation.clone(),
                        });
                        if result.was_modified {
                            result.code
                        } else {
                            code
                        }
                    }
                    Err(e) => {
                        eprintln!("Code review failed: {}", e);
                        code
                    }
                }
            } else {
                code
            };

            let _ = on_event.send(MultiPartEvent::FinalCode { code: final_code.clone() });
            let _ = on_event.send(MultiPartEvent::Done {
                success: true,
                error: None,
            });
            Ok(final_code)
        }
        Err(e) => {
            let _ = on_event.send(MultiPartEvent::Done {
                success: false,
                error: Some(e.clone()),
            });
            Err(AppError::AiProviderError(e))
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse the planner JSON response. Falls back to single mode on any parse failure.
fn parse_plan(json_str: &str) -> GenerationPlan {
    // Try to extract JSON from the response (the AI might wrap it in markdown fences)
    let cleaned = json_str
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    serde_json::from_str::<GenerationPlan>(cleaned).unwrap_or_else(|_| GenerationPlan {
        mode: "single".to_string(),
        description: None,
        parts: vec![],
    })
}

/// Extract a Python code block from an AI response.
fn extract_code_from_response(response: &str) -> Option<String> {
    let re = Regex::new(r"```python\s*\n([\s\S]*?)```").ok()?;
    re.captures(response).map(|cap| cap[1].trim().to_string())
}
