use regex::Regex;
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::State;
use tokio::sync::mpsc;

use crate::ai::message::ChatMessage;
use crate::ai::cost;
use crate::ai::provider::{StreamDelta, TokenUsage};
use crate::agent::consensus;
use crate::agent::design;
use crate::agent::executor;
use crate::agent::iterative;
use crate::agent::modify;
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
        stl_base64: Option<String>,
    },
    ReviewStatus {
        message: String,
    },
    ReviewComplete {
        was_modified: bool,
        explanation: String,
    },
    TokenUsage {
        phase: String,
        input_tokens: u32,
        output_tokens: u32,
        total_tokens: u32,
        cost_usd: Option<f64>,
    },
    ValidationAttempt {
        attempt: u32,
        max_attempts: u32,
        message: String,
    },
    ValidationSuccess {
        attempt: u32,
        message: String,
    },
    ValidationFailed {
        attempt: u32,
        error_category: String,
        error_message: String,
        will_retry: bool,
    },
    IterativeStart {
        total_steps: usize,
        steps: Vec<iterative::BuildStep>,
    },
    IterativeStepStarted {
        step_index: usize,
        step_name: String,
        description: String,
    },
    IterativeStepComplete {
        step_index: usize,
        success: bool,
        stl_base64: Option<String>,
    },
    IterativeStepRetry {
        step_index: usize,
        attempt: u32,
        error: String,
    },
    IterativeStepSkipped {
        step_index: usize,
        name: String,
        error: String,
    },
    IterativeComplete {
        final_code: String,
        stl_base64: Option<String>,
        skipped_steps: Vec<iterative::SkippedStep>,
    },
    ModificationDetected {
        intent_summary: String,
    },
    CodeDiff {
        diff_lines: Vec<crate::agent::modify::DiffLine>,
        old_line_count: usize,
        new_line_count: usize,
        additions: usize,
        deletions: usize,
    },
    ConsensusStarted {
        candidate_count: u32,
    },
    ConsensusCandidate {
        label: String,
        temperature: f32,
        status: String,
        has_code: Option<bool>,
        execution_success: Option<bool>,
    },
    ConsensusWinner {
        label: String,
        score: u32,
        reason: String,
    },
    Done {
        success: bool,
        error: Option<String>,
        validated: bool,
    },
}

#[derive(Clone, Serialize)]
pub struct DesignPlanResult {
    pub plan_text: String,
    pub risk_score: u32,
    pub warnings: Vec<String>,
    pub is_valid: bool,
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
        Wrap your code in <CODE>...</CODE> tags (```python fences also accepted).",
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
// Token usage helper
// ---------------------------------------------------------------------------

fn emit_usage(
    on_event: &Channel<MultiPartEvent>,
    phase: &str,
    usage: &TokenUsage,
    provider: &str,
    model: &str,
) {
    let cost_usd = cost::estimate_cost(provider, model, usage);
    let _ = on_event.send(MultiPartEvent::TokenUsage {
        phase: phase.to_string(),
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        total_tokens: usage.total(),
        cost_usd,
    });
}

// ---------------------------------------------------------------------------
// Extracted helpers (shared by generate_parallel, generate_design_plan, generate_from_plan)
// ---------------------------------------------------------------------------

/// Phase 0: Generate and validate the geometry design plan.
async fn run_design_plan_phase(
    message: &str,
    config: &crate::config::AppConfig,
    on_event: &Channel<MultiPartEvent>,
    total_usage: &mut TokenUsage,
    provider_id: &str,
    model_id: &str,
) -> Result<(design::DesignPlan, DesignPlanResult), AppError> {
    let _ = on_event.send(MultiPartEvent::PlanStatus {
        message: "Designing geometry...".to_string(),
    });

    let design_extra_context = {
        let rules =
            crate::agent::rules::AgentRules::from_preset(config.agent_rules_preset.as_deref())
                .ok();
        let mut ctx = String::new();
        if let Some(ref r) = rules {
            if let Some(ref m) = r.manufacturing {
                ctx.push_str(&crate::agent::design::format_manufacturing_constraints(m));
            }
            if let Some(ref d) = r.dimension_guidance {
                if !ctx.is_empty() {
                    ctx.push_str("\n\n");
                }
                ctx.push_str(&crate::agent::design::format_dimension_guidance(d));
            }
            if let Some(ref fp) = r.failure_prevention {
                if !ctx.is_empty() {
                    ctx.push_str("\n\n");
                }
                ctx.push_str(&crate::agent::design::format_failure_prevention(fp));
            }
        }
        if ctx.is_empty() {
            None
        } else {
            Some(ctx)
        }
    };

    let design_provider = create_provider(config)?;
    let (mut design_plan, design_usage) =
        design::plan_geometry(design_provider, message, design_extra_context.as_deref()).await?;
    if let Some(ref u) = design_usage {
        total_usage.add(u);
        emit_usage(on_event, "design", u, provider_id, model_id);
    }

    let validation = design::validate_plan(&design_plan.text);

    let _ = on_event.send(MultiPartEvent::PlanValidation {
        risk_score: validation.risk_score,
        warnings: validation.warnings.clone(),
        is_valid: validation.is_valid,
        rejected_reason: validation.rejected_reason.clone(),
    });

    let mut final_risk_score = validation.risk_score;
    let mut final_warnings = validation.warnings.clone();
    let mut final_is_valid = validation.is_valid;

    if !validation.is_valid {
        let _ = on_event.send(MultiPartEvent::PlanStatus {
            message: format!(
                "Design plan too risky (score {}/10), re-planning...",
                validation.risk_score
            ),
        });

        let feedback = design::build_rejection_feedback(&validation);
        let retry_provider = create_provider(config)?;
        let (retry_plan, retry_usage) = design::plan_geometry_with_feedback(
            retry_provider,
            message,
            &feedback,
            design_extra_context.as_deref(),
        )
        .await?;
        design_plan = retry_plan;
        if let Some(ref u) = retry_usage {
            total_usage.add(u);
            emit_usage(on_event, "design", u, provider_id, model_id);
        }

        let retry_validation = design::validate_plan(&design_plan.text);
        final_risk_score = retry_validation.risk_score;
        final_warnings = retry_validation.warnings.clone();
        final_is_valid = retry_validation.is_valid;
        let _ = on_event.send(MultiPartEvent::PlanValidation {
            risk_score: retry_validation.risk_score,
            warnings: retry_validation.warnings.clone(),
            is_valid: retry_validation.is_valid,
            rejected_reason: retry_validation.rejected_reason.clone(),
        });
    }

    let _ = on_event.send(MultiPartEvent::DesignPlan {
        plan_text: design_plan.text.clone(),
    });

    let result = DesignPlanResult {
        plan_text: design_plan.text.clone(),
        risk_score: final_risk_score,
        warnings: final_warnings,
        is_valid: final_is_valid,
    };

    Ok((design_plan, result))
}

/// Phase 1+: Planner decomposition, code generation (single/multi/iterative/consensus),
/// review, and validation. Returns the final AI response string.
#[allow(clippy::too_many_arguments)]
async fn run_generation_pipeline(
    plan_text: &str,
    user_request: &str,
    history: Vec<ChatMessage>,
    config: &crate::config::AppConfig,
    system_prompt: &str,
    on_event: &Channel<MultiPartEvent>,
    execution_ctx: Option<&executor::ExecutionContext>,
    total_usage: &mut TokenUsage,
    provider_id: &str,
    model_id: &str,
) -> Result<String, AppError> {
    let enhanced_message = format!(
        "## Geometry Design Plan\n{}\n\n## User Request\n{}",
        plan_text, user_request
    );

    // -----------------------------------------------------------------------
    // Phase 1: Plan (decomposition)
    // -----------------------------------------------------------------------
    let _ = on_event.send(MultiPartEvent::PlanStatus {
        message: "Analyzing request...".to_string(),
    });

    let planner = create_provider(config)?;

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

    let (plan_json, plan_usage) = planner.complete(&planner_messages, Some(1024)).await?;
    if let Some(ref u) = plan_usage {
        total_usage.add(u);
        emit_usage(on_event, "plan", u, provider_id, model_id);
    }

    let plan: GenerationPlan = parse_plan(&plan_json);

    let _ = on_event.send(MultiPartEvent::PlanResult {
        plan: plan.clone(),
    });

    // -----------------------------------------------------------------------
    // Single mode: fall through to normal streaming
    // -----------------------------------------------------------------------
    if plan.mode == "single" || plan.parts.is_empty() {
        // Check if iterative mode should be used
        let build_steps = iterative::parse_build_steps(plan_text);

        if iterative::should_use_iterative(&build_steps) {
            if let Some(ctx) = execution_ctx {
                let _ = on_event.send(MultiPartEvent::PlanStatus {
                    message: format!(
                        "Building step by step ({} steps)...",
                        build_steps.len()
                    ),
                });

                let on_iter_event = |evt: iterative::IterativeEvent| match evt {
                    iterative::IterativeEvent::Start { total_steps, steps } => {
                        let _ = on_event.send(MultiPartEvent::IterativeStart {
                            total_steps,
                            steps,
                        });
                    }
                    iterative::IterativeEvent::StepStarted {
                        step_index,
                        step_name,
                        description,
                    } => {
                        let _ = on_event.send(MultiPartEvent::IterativeStepStarted {
                            step_index,
                            step_name,
                            description,
                        });
                    }
                    iterative::IterativeEvent::StepComplete {
                        step_index,
                        success,
                        stl_base64,
                    } => {
                        let _ = on_event.send(MultiPartEvent::IterativeStepComplete {
                            step_index,
                            success,
                            stl_base64,
                        });
                    }
                    iterative::IterativeEvent::StepRetry {
                        step_index,
                        attempt,
                        error,
                    } => {
                        let _ = on_event.send(MultiPartEvent::IterativeStepRetry {
                            step_index,
                            attempt,
                            error,
                        });
                    }
                    iterative::IterativeEvent::StepSkipped {
                        step_index,
                        name,
                        error,
                    } => {
                        let _ = on_event.send(MultiPartEvent::IterativeStepSkipped {
                            step_index,
                            name,
                            error,
                        });
                    }
                };

                let result = iterative::run_iterative_build(
                    &build_steps,
                    plan_text,
                    user_request,
                    system_prompt,
                    config,
                    ctx,
                    &on_iter_event,
                )
                .await?;

                total_usage.add(&result.total_usage);
                if result.total_usage.total() > 0 {
                    emit_usage(
                        on_event,
                        "iterative",
                        &result.total_usage,
                        provider_id,
                        model_id,
                    );
                }

                let _ = on_event.send(MultiPartEvent::FinalCode {
                    code: result.final_code.clone(),
                    stl_base64: result.stl_base64.clone(),
                });

                let _ = on_event.send(MultiPartEvent::IterativeComplete {
                    final_code: result.final_code.clone(),
                    stl_base64: result.stl_base64.clone(),
                    skipped_steps: result.skipped_steps.clone(),
                });

                if total_usage.total() > 0 {
                    emit_usage(on_event, "total", total_usage, provider_id, model_id);
                }

                let _ = on_event.send(MultiPartEvent::Done {
                    success: result.success,
                    error: if result.success {
                        None
                    } else {
                        Some("Iterative build failed".to_string())
                    },
                    validated: true,
                });

                return Ok(result.final_code);
            }
            // No Python execution context → fall through to single-shot
        }

        // -------------------------------------------------------------------
        // Consensus branch
        // -------------------------------------------------------------------
        if config.enable_consensus {
            if let Some(ctx) = execution_ctx {
                let _ = on_event.send(MultiPartEvent::PlanStatus {
                    message: "Running consensus (2 candidates)...".to_string(),
                });

                let mut consensus_messages = vec![ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                }];
                consensus_messages.extend(history.clone());
                consensus_messages.push(ChatMessage {
                    role: "user".to_string(),
                    content: enhanced_message.clone(),
                });

                let on_consensus_event = |evt: consensus::ConsensusEvent| match evt {
                    consensus::ConsensusEvent::Started { candidate_count } => {
                        let _ = on_event
                            .send(MultiPartEvent::ConsensusStarted { candidate_count });
                    }
                    consensus::ConsensusEvent::CandidateUpdate {
                        label,
                        temperature,
                        status,
                        has_code,
                        execution_success,
                    } => {
                        let _ = on_event.send(MultiPartEvent::ConsensusCandidate {
                            label,
                            temperature,
                            status,
                            has_code,
                            execution_success,
                        });
                    }
                    consensus::ConsensusEvent::Winner {
                        label,
                        score,
                        reason,
                    } => {
                        let _ = on_event.send(MultiPartEvent::ConsensusWinner {
                            label,
                            score,
                            reason,
                        });
                    }
                };

                let consensus_result =
                    consensus::run_consensus(&consensus_messages, config, ctx, &on_consensus_event)
                        .await?;

                total_usage.add(&consensus_result.total_usage);
                if consensus_result.total_usage.total() > 0 {
                    emit_usage(
                        on_event,
                        "consensus",
                        &consensus_result.total_usage,
                        provider_id,
                        model_id,
                    );
                }

                let winner = &consensus_result.winner;

                if let Some(ref code) = winner.code {
                    let response_text = winner.response.clone().unwrap_or_default();
                    let _ = on_event.send(MultiPartEvent::SingleDone {
                        full_response: response_text.clone(),
                    });

                    let mut final_code = code.clone();
                    let mut reviewed = false;
                    if config.enable_code_review {
                        let _ = on_event.send(MultiPartEvent::ReviewStatus {
                            message: "Reviewing consensus winner...".to_string(),
                        });
                        let review_provider = create_provider(config)?;
                        match review::review_code(
                            review_provider,
                            user_request,
                            code,
                            Some(plan_text),
                        )
                        .await
                        {
                            Ok((result, review_usage)) => {
                                if let Some(ref u) = review_usage {
                                    total_usage.add(u);
                                    emit_usage(on_event, "review", u, provider_id, model_id);
                                }
                                let _ = on_event.send(MultiPartEvent::ReviewComplete {
                                    was_modified: result.was_modified,
                                    explanation: result.explanation.clone(),
                                });
                                if result.was_modified {
                                    final_code = result.code;
                                    reviewed = true;
                                }
                            }
                            Err(e) => {
                                eprintln!("Code review failed (consensus): {}", e);
                            }
                        }
                    }

                    if winner.execution_success && !reviewed {
                        let _ = on_event.send(MultiPartEvent::FinalCode {
                            code: final_code.clone(),
                            stl_base64: winner.stl_base64.clone(),
                        });
                    } else {
                        let on_validation_event = |evt: executor::ValidationEvent| match evt {
                            executor::ValidationEvent::Attempt {
                                attempt,
                                max_attempts,
                                message,
                            } => {
                                let _ = on_event.send(MultiPartEvent::ValidationAttempt {
                                    attempt,
                                    max_attempts,
                                    message,
                                });
                            }
                            executor::ValidationEvent::Success { attempt, message } => {
                                let _ = on_event.send(MultiPartEvent::ValidationSuccess {
                                    attempt,
                                    message,
                                });
                            }
                            executor::ValidationEvent::Failed {
                                attempt,
                                error_category,
                                error_message,
                                will_retry,
                            } => {
                                let _ = on_event.send(MultiPartEvent::ValidationFailed {
                                    attempt,
                                    error_category,
                                    error_message,
                                    will_retry,
                                });
                            }
                        };

                        let validation_result = executor::validate_and_retry(
                            final_code.clone(),
                            ctx,
                            system_prompt,
                            &on_validation_event,
                        )
                        .await?;

                        if validation_result.retry_usage.total() > 0 {
                            total_usage.add(&validation_result.retry_usage);
                            emit_usage(
                                on_event,
                                "validation",
                                &validation_result.retry_usage,
                                provider_id,
                                model_id,
                            );
                        }

                        let _ = on_event.send(MultiPartEvent::FinalCode {
                            code: validation_result.code.clone(),
                            stl_base64: validation_result.stl_base64.clone(),
                        });
                    }

                    if total_usage.total() > 0 {
                        emit_usage(on_event, "total", total_usage, provider_id, model_id);
                    }

                    let _ = on_event.send(MultiPartEvent::Done {
                        success: true,
                        error: None,
                        validated: true,
                    });

                    return Ok(response_text);
                }

                // Consensus failed — fall through
                let _ = on_event.send(MultiPartEvent::PlanStatus {
                    message: "Consensus failed to produce code, falling back to single generation..."
                        .to_string(),
                });
            }
            // No execution context — fall through to single-shot
        }

        let _ = on_event.send(MultiPartEvent::PlanStatus {
            message: "Generating code...".to_string(),
        });

        let provider = create_provider(config)?;

        let mut messages_list = vec![ChatMessage {
            role: "system".to_string(),
            content: system_prompt.to_string(),
        }];
        messages_list.extend(history);
        messages_list.push(ChatMessage {
            role: "user".to_string(),
            content: enhanced_message.clone(),
        });

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
            Ok(Ok(stream_usage)) => {
                if let Some(ref u) = stream_usage {
                    total_usage.add(u);
                    emit_usage(on_event, "generate", u, provider_id, model_id);
                }
            }
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

        let mut final_code = extract_code_from_response(&full_response);
        let mut final_response = full_response.clone();

        if config.enable_code_review {
            if let Some(ref code) = final_code {
                let _ = on_event.send(MultiPartEvent::ReviewStatus {
                    message: "Reviewing generated code...".to_string(),
                });

                let review_provider = create_provider(config)?;
                match review::review_code(review_provider, user_request, code, Some(plan_text))
                    .await
                {
                    Ok((result, review_usage)) => {
                        if let Some(ref u) = review_usage {
                            total_usage.add(u);
                            emit_usage(on_event, "review", u, provider_id, model_id);
                        }
                        let _ = on_event.send(MultiPartEvent::ReviewComplete {
                            was_modified: result.was_modified,
                            explanation: result.explanation.clone(),
                        });
                        if result.was_modified {
                            final_response = full_response.replace(code, &result.code);
                            final_code = Some(result.code);
                        }
                    }
                    Err(e) => {
                        eprintln!("Code review failed: {}", e);
                    }
                }
            }
        }

        // Backend validation
        if let (Some(code), Some(ctx)) = (&final_code, execution_ctx) {
            let on_validation_event = |evt: executor::ValidationEvent| match evt {
                executor::ValidationEvent::Attempt {
                    attempt,
                    max_attempts,
                    message,
                } => {
                    let _ = on_event.send(MultiPartEvent::ValidationAttempt {
                        attempt,
                        max_attempts,
                        message,
                    });
                }
                executor::ValidationEvent::Success { attempt, message } => {
                    let _ = on_event.send(MultiPartEvent::ValidationSuccess {
                        attempt,
                        message,
                    });
                }
                executor::ValidationEvent::Failed {
                    attempt,
                    error_category,
                    error_message,
                    will_retry,
                } => {
                    let _ = on_event.send(MultiPartEvent::ValidationFailed {
                        attempt,
                        error_category,
                        error_message,
                        will_retry,
                    });
                }
            };

            let validation_result = executor::validate_and_retry(
                code.clone(),
                ctx,
                system_prompt,
                &on_validation_event,
            )
            .await?;

            if validation_result.retry_usage.total() > 0 {
                total_usage.add(&validation_result.retry_usage);
                emit_usage(
                    on_event,
                    "validation",
                    &validation_result.retry_usage,
                    provider_id,
                    model_id,
                );
            }

            let _ = on_event.send(MultiPartEvent::FinalCode {
                code: validation_result.code.clone(),
                stl_base64: validation_result.stl_base64.clone(),
            });

            if validation_result.code != *code {
                final_response = final_response.replace(code, &validation_result.code);
            }

            if total_usage.total() > 0 {
                emit_usage(on_event, "total", total_usage, provider_id, model_id);
            }

            let _ = on_event.send(MultiPartEvent::Done {
                success: validation_result.success,
                error: validation_result.error,
                validated: true,
            });

            return Ok(final_response);
        }

        // No execution context — emit as-is
        if let Some(ref code) = final_code {
            let _ = on_event.send(MultiPartEvent::FinalCode {
                code: code.clone(),
                stl_base64: None,
            });
        }

        if total_usage.total() > 0 {
            emit_usage(on_event, "total", total_usage, provider_id, model_id);
        }

        let _ = on_event.send(MultiPartEvent::Done {
            success: true,
            error: None,
            validated: false,
        });

        return Ok(final_response);
    }

    // -----------------------------------------------------------------------
    // Phase 2: Parallel generation
    // -----------------------------------------------------------------------
    let _ = on_event.send(MultiPartEvent::PlanStatus {
        message: format!("Generating {} parts in parallel...", plan.parts.len()),
    });

    let mut handles = Vec::new();

    for (idx, part) in plan.parts.iter().enumerate() {
        let part_provider = create_provider(config)?;
        let part_prompt = build_part_prompt(system_prompt, part, plan_text);
        let part_name = part.name.clone();
        let event_channel = on_event.clone();

        let part_messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: part_prompt,
            },
            ChatMessage {
                role: "user".to_string(),
                content: format!("Generate the CadQuery code for: {}", part.description),
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
                Ok(Ok(usage)) => Ok((full_response, usage)),
                Ok(Err(e)) => Err(e.to_string()),
                Err(e) => Err(format!("Part task panicked: {}", e)),
            };

            (idx, result)
        });

        handles.push((idx, part.name.clone(), handle));
    }

    // Collect results
    let mut part_codes: Vec<Option<(String, String, [f64; 3])>> = vec![None; plan.parts.len()];
    let mut any_success = false;

    for (idx, name, handle) in handles {
        let position = plan.parts[idx].position;

        match handle.await {
            Ok((_, Ok((response, part_usage)))) => {
                if let Some(ref u) = part_usage {
                    total_usage.add(u);
                }
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

    if total_usage.total() > 0 {
        emit_usage(on_event, "generate", total_usage, provider_id, model_id);
    }

    if !any_success {
        let _ = on_event.send(MultiPartEvent::Done {
            success: false,
            error: Some("All parts failed to generate".to_string()),
            validated: false,
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

    let successful_parts: Vec<(String, String, [f64; 3])> =
        part_codes.into_iter().flatten().collect();

    match assemble_parts(&successful_parts) {
        Ok(code) => {
            let final_code = if config.enable_code_review {
                let _ = on_event.send(MultiPartEvent::ReviewStatus {
                    message: "Reviewing assembled code...".to_string(),
                });
                let review_provider = create_provider(config)?;
                match review::review_code(review_provider, user_request, &code, Some(plan_text))
                    .await
                {
                    Ok((result, review_usage)) => {
                        if let Some(ref u) = review_usage {
                            total_usage.add(u);
                            emit_usage(on_event, "review", u, provider_id, model_id);
                        }
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

            if let Some(ctx) = execution_ctx {
                let on_validation_event = |evt: executor::ValidationEvent| match evt {
                    executor::ValidationEvent::Attempt {
                        attempt,
                        max_attempts,
                        message,
                    } => {
                        let _ = on_event.send(MultiPartEvent::ValidationAttempt {
                            attempt,
                            max_attempts,
                            message,
                        });
                    }
                    executor::ValidationEvent::Success { attempt, message } => {
                        let _ = on_event.send(MultiPartEvent::ValidationSuccess {
                            attempt,
                            message,
                        });
                    }
                    executor::ValidationEvent::Failed {
                        attempt,
                        error_category,
                        error_message,
                        will_retry,
                    } => {
                        let _ = on_event.send(MultiPartEvent::ValidationFailed {
                            attempt,
                            error_category,
                            error_message,
                            will_retry,
                        });
                    }
                };

                let validation_result = executor::validate_and_retry(
                    final_code.clone(),
                    ctx,
                    system_prompt,
                    &on_validation_event,
                )
                .await?;

                if validation_result.retry_usage.total() > 0 {
                    total_usage.add(&validation_result.retry_usage);
                    emit_usage(
                        on_event,
                        "validation",
                        &validation_result.retry_usage,
                        provider_id,
                        model_id,
                    );
                }

                let _ = on_event.send(MultiPartEvent::FinalCode {
                    code: validation_result.code.clone(),
                    stl_base64: validation_result.stl_base64.clone(),
                });

                if total_usage.total() > 0 {
                    emit_usage(on_event, "total", total_usage, provider_id, model_id);
                }

                let _ = on_event.send(MultiPartEvent::Done {
                    success: validation_result.success,
                    error: validation_result.error,
                    validated: true,
                });

                return Ok(validation_result.code);
            }

            // No execution context — emit as-is
            if total_usage.total() > 0 {
                emit_usage(on_event, "total", total_usage, provider_id, model_id);
            }

            let _ = on_event.send(MultiPartEvent::FinalCode {
                code: final_code.clone(),
                stl_base64: None,
            });
            let _ = on_event.send(MultiPartEvent::Done {
                success: true,
                error: None,
                validated: false,
            });
            Ok(final_code)
        }
        Err(e) => {
            let _ = on_event.send(MultiPartEvent::Done {
                success: false,
                error: Some(e.clone()),
                validated: false,
            });
            Err(AppError::AiProviderError(e))
        }
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn generate_parallel(
    message: String,
    history: Vec<ChatMessage>,
    existing_code: Option<String>,
    on_event: Channel<MultiPartEvent>,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let config = state.config.lock().unwrap().clone();
    let system_prompt =
        prompts::build_system_prompt_for_preset(config.agent_rules_preset.as_deref());
    let user_request = message.clone();

    let provider_id = config.ai_provider.clone();
    let model_id = config.model.clone();
    let mut total_usage = TokenUsage::default();

    // Resolve execution context for backend validation (None if Python not set up)
    let execution_ctx = {
        let venv_path = state.venv_path.lock().unwrap().clone();
        match venv_path {
            Some(venv_dir) => {
                match super::find_python_script("runner.py") {
                    Ok(runner_script) => Some(executor::ExecutionContext {
                        venv_dir,
                        runner_script,
                        config: config.clone(),
                    }),
                    Err(_) => None,
                }
            }
            None => None,
        }
    };

    // -----------------------------------------------------------------------
    // Modification branch: detect and handle code modifications (early return)
    // -----------------------------------------------------------------------
    let modification_intent = modify::detect_modification_intent(
        &message,
        existing_code.as_deref(),
    );

    if modification_intent.is_modification {
        let intent_summary = modification_intent
            .intent_summary
            .unwrap_or_else(|| "modifying code".to_string());

        let _ = on_event.send(MultiPartEvent::ModificationDetected {
            intent_summary: intent_summary.clone(),
        });

        let _ = on_event.send(MultiPartEvent::PlanStatus {
            message: "Modifying existing code...".to_string(),
        });

        let old_code = existing_code.as_deref().unwrap_or("");

        // Build modification-specific system prompt and user message
        let mod_system_prompt = format!("{}\n{}", system_prompt, modify::MODIFICATION_INSTRUCTIONS);
        let modification_message = modify::build_modification_message(old_code, &message);

        let provider = create_provider(&config)?;
        let mut messages_list = vec![ChatMessage {
            role: "system".to_string(),
            content: mod_system_prompt,
        }];
        messages_list.extend(history);
        messages_list.push(ChatMessage {
            role: "user".to_string(),
            content: modification_message,
        });

        // Stream the AI response (reuse SingleDelta/SingleDone events)
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
            Ok(Ok(stream_usage)) => {
                if let Some(ref u) = stream_usage {
                    total_usage.add(u);
                    emit_usage(&on_event, "generate", u, &provider_id, &model_id);
                }
            }
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

        // Extract code from the response
        let mut final_code = extract_code_from_response(&full_response);
        let mut final_response = full_response.clone();

        // Optional: code review
        if config.enable_code_review {
            if let Some(ref code) = final_code {
                let _ = on_event.send(MultiPartEvent::ReviewStatus {
                    message: "Reviewing modified code...".to_string(),
                });

                let review_provider = create_provider(&config)?;
                match review::review_code(review_provider, &user_request, code, None).await {
                    Ok((result, review_usage)) => {
                        if let Some(ref u) = review_usage {
                            total_usage.add(u);
                            emit_usage(&on_event, "review", u, &provider_id, &model_id);
                        }
                        let _ = on_event.send(MultiPartEvent::ReviewComplete {
                            was_modified: result.was_modified,
                            explanation: result.explanation.clone(),
                        });
                        if result.was_modified {
                            final_response = full_response.replace(code, &result.code);
                            final_code = Some(result.code);
                        }
                    }
                    Err(e) => {
                        eprintln!("Code review failed (modification): {}", e);
                    }
                }
            }
        }

        // Optional: backend validation
        if let (Some(code), Some(ref ctx)) = (&final_code, &execution_ctx) {
            let on_validation_event = |evt: executor::ValidationEvent| {
                match evt {
                    executor::ValidationEvent::Attempt { attempt, max_attempts, message } => {
                        let _ = on_event.send(MultiPartEvent::ValidationAttempt {
                            attempt, max_attempts, message,
                        });
                    }
                    executor::ValidationEvent::Success { attempt, message } => {
                        let _ = on_event.send(MultiPartEvent::ValidationSuccess {
                            attempt, message,
                        });
                    }
                    executor::ValidationEvent::Failed { attempt, error_category, error_message, will_retry } => {
                        let _ = on_event.send(MultiPartEvent::ValidationFailed {
                            attempt, error_category, error_message, will_retry,
                        });
                    }
                }
            };

            let validation_result = executor::validate_and_retry(
                code.clone(), ctx, &system_prompt, &on_validation_event,
            ).await?;

            if validation_result.retry_usage.total() > 0 {
                total_usage.add(&validation_result.retry_usage);
                emit_usage(&on_event, "validation", &validation_result.retry_usage, &provider_id, &model_id);
            }

            let new_code = &validation_result.code;

            // Compute diff between old code and final new code
            let diff = modify::compute_diff(old_code, new_code);
            if modify::diff_has_changes(&diff) {
                let additions = diff.iter().filter(|l| l.tag == "insert").count();
                let deletions = diff.iter().filter(|l| l.tag == "delete").count();
                let _ = on_event.send(MultiPartEvent::CodeDiff {
                    diff_lines: diff,
                    old_line_count: old_code.lines().count(),
                    new_line_count: new_code.lines().count(),
                    additions,
                    deletions,
                });
            }

            let _ = on_event.send(MultiPartEvent::FinalCode {
                code: validation_result.code.clone(),
                stl_base64: validation_result.stl_base64.clone(),
            });

            if total_usage.total() > 0 {
                emit_usage(&on_event, "total", &total_usage, &provider_id, &model_id);
            }

            let _ = on_event.send(MultiPartEvent::Done {
                success: validation_result.success,
                error: validation_result.error,
                validated: true,
            });

            return Ok(final_response);
        }

        // No execution context — emit diff and code as-is
        if let Some(ref code) = final_code {
            let diff = modify::compute_diff(old_code, code);
            if modify::diff_has_changes(&diff) {
                let additions = diff.iter().filter(|l| l.tag == "insert").count();
                let deletions = diff.iter().filter(|l| l.tag == "delete").count();
                let _ = on_event.send(MultiPartEvent::CodeDiff {
                    diff_lines: diff,
                    old_line_count: old_code.lines().count(),
                    new_line_count: code.lines().count(),
                    additions,
                    deletions,
                });
            }

            let _ = on_event.send(MultiPartEvent::FinalCode {
                code: code.clone(),
                stl_base64: None,
            });
        }

        if total_usage.total() > 0 {
            emit_usage(&on_event, "total", &total_usage, &provider_id, &model_id);
        }

        let _ = on_event.send(MultiPartEvent::Done {
            success: true,
            error: None,
            validated: false,
        });

        return Ok(final_response);
    }

    // -----------------------------------------------------------------------
    // Phase 0: Geometry Design Plan (always runs)
    // -----------------------------------------------------------------------
    let (design_plan, _plan_result) = run_design_plan_phase(
        &message,
        &config,
        &on_event,
        &mut total_usage,
        &provider_id,
        &model_id,
    )
    .await?;

    // -----------------------------------------------------------------------
    // Phase 1+: Generation pipeline (planner, code gen, review, validation)
    // -----------------------------------------------------------------------
    run_generation_pipeline(
        &design_plan.text,
        &user_request,
        history,
        &config,
        &system_prompt,
        &on_event,
        execution_ctx.as_ref(),
        &mut total_usage,
        &provider_id,
        &model_id,
    )
    .await
}

// ---------------------------------------------------------------------------
// New commands for two-phase plan flow
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn generate_design_plan(
    message: String,
    _history: Vec<ChatMessage>,
    on_event: Channel<MultiPartEvent>,
    state: State<'_, AppState>,
) -> Result<DesignPlanResult, AppError> {
    let config = state.config.lock().unwrap().clone();
    let provider_id = config.ai_provider.clone();
    let model_id = config.model.clone();
    let mut total_usage = TokenUsage::default();

    let (_design_plan, plan_result) = run_design_plan_phase(
        &message,
        &config,
        &on_event,
        &mut total_usage,
        &provider_id,
        &model_id,
    )
    .await?;

    if total_usage.total() > 0 {
        emit_usage(&on_event, "total", &total_usage, &provider_id, &model_id);
    }

    Ok(plan_result)
}

#[tauri::command]
pub async fn generate_from_plan(
    plan_text: String,
    user_request: String,
    history: Vec<ChatMessage>,
    existing_code: Option<String>,
    on_event: Channel<MultiPartEvent>,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let _ = existing_code; // reserved for future use
    let config = state.config.lock().unwrap().clone();
    let system_prompt =
        prompts::build_system_prompt_for_preset(config.agent_rules_preset.as_deref());
    let provider_id = config.ai_provider.clone();
    let model_id = config.model.clone();
    let mut total_usage = TokenUsage::default();

    let execution_ctx = {
        let venv_path = state.venv_path.lock().unwrap().clone();
        match venv_path {
            Some(venv_dir) => match super::find_python_script("runner.py") {
                Ok(runner_script) => Some(executor::ExecutionContext {
                    venv_dir,
                    runner_script,
                    config: config.clone(),
                }),
                Err(_) => None,
            },
            None => None,
        }
    };

    run_generation_pipeline(
        &plan_text,
        &user_request,
        history,
        &config,
        &system_prompt,
        &on_event,
        execution_ctx.as_ref(),
        &mut total_usage,
        &provider_id,
        &model_id,
    )
    .await
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
    crate::agent::extract::extract_code(response)
}

// ---------------------------------------------------------------------------
// Retry skipped steps command
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn retry_skipped_steps(
    current_code: String,
    skipped_steps: Vec<iterative::SkippedStep>,
    design_plan_text: String,
    user_request: String,
    on_event: Channel<MultiPartEvent>,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let config = state.config.lock().unwrap().clone();
    let system_prompt =
        crate::agent::prompts::build_system_prompt_for_preset(config.agent_rules_preset.as_deref());

    let provider_id = config.ai_provider.clone();
    let model_id = config.model.clone();
    let mut total_usage = TokenUsage::default();

    // Resolve execution context
    let execution_ctx = {
        let venv_path = state.venv_path.lock().unwrap().clone();
        match venv_path {
            Some(venv_dir) => match super::find_python_script("runner.py") {
                Ok(runner_script) => Some(executor::ExecutionContext {
                    venv_dir,
                    runner_script,
                    config: config.clone(),
                }),
                Err(_) => None,
            },
            None => None,
        }
    };

    let ctx = execution_ctx.ok_or_else(|| {
        AppError::CadQueryError("Python environment not available for retry".to_string())
    })?;

    // Convert SkippedSteps back to BuildSteps
    let build_steps: Vec<iterative::BuildStep> = skipped_steps
        .iter()
        .map(|s| iterative::BuildStep {
            index: s.step_index,
            name: s.name.clone(),
            description: s.description.clone(),
            operations: crate::agent::design::extract_operations_from_text(&s.description),
        })
        .collect();

    let on_iter_event = |evt: iterative::IterativeEvent| {
        match evt {
            iterative::IterativeEvent::Start { total_steps, steps } => {
                let _ = on_event.send(MultiPartEvent::IterativeStart { total_steps, steps });
            }
            iterative::IterativeEvent::StepStarted {
                step_index,
                step_name,
                description,
            } => {
                let _ = on_event.send(MultiPartEvent::IterativeStepStarted {
                    step_index,
                    step_name,
                    description,
                });
            }
            iterative::IterativeEvent::StepComplete {
                step_index,
                success,
                stl_base64,
            } => {
                let _ = on_event.send(MultiPartEvent::IterativeStepComplete {
                    step_index,
                    success,
                    stl_base64,
                });
            }
            iterative::IterativeEvent::StepRetry {
                step_index,
                attempt,
                error,
            } => {
                let _ = on_event.send(MultiPartEvent::IterativeStepRetry {
                    step_index,
                    attempt,
                    error,
                });
            }
            iterative::IterativeEvent::StepSkipped {
                step_index,
                name,
                error,
            } => {
                let _ = on_event.send(MultiPartEvent::IterativeStepSkipped {
                    step_index,
                    name,
                    error,
                });
            }
        }
    };

    let result = iterative::run_iterative_build_from(
        &build_steps,
        &current_code,
        &design_plan_text,
        &user_request,
        &system_prompt,
        &config,
        &ctx,
        &on_iter_event,
    )
    .await?;

    total_usage.add(&result.total_usage);
    if result.total_usage.total() > 0 {
        emit_usage(
            &on_event,
            "iterative_retry",
            &result.total_usage,
            &provider_id,
            &model_id,
        );
    }

    let _ = on_event.send(MultiPartEvent::FinalCode {
        code: result.final_code.clone(),
        stl_base64: result.stl_base64.clone(),
    });

    let _ = on_event.send(MultiPartEvent::IterativeComplete {
        final_code: result.final_code.clone(),
        stl_base64: result.stl_base64.clone(),
        skipped_steps: result.skipped_steps.clone(),
    });

    if total_usage.total() > 0 {
        emit_usage(&on_event, "total", &total_usage, &provider_id, &model_id);
    }

    let _ = on_event.send(MultiPartEvent::Done {
        success: result.success,
        error: if result.success {
            None
        } else {
            Some("Retry of skipped steps failed".to_string())
        },
        validated: true,
    });

    Ok(result.final_code)
}
