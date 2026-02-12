use regex::Regex;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::ipc::Channel;
use tauri::State;
use tokio::sync::mpsc;
use tokio::time::timeout;

use crate::agent::confidence;
use crate::agent::consensus;
use crate::agent::design;
use crate::agent::executor;
use crate::agent::iterative;
use crate::agent::memory;
use crate::agent::modify;
use crate::agent::prompts;
use crate::agent::retrieval;
use crate::agent::review;
use crate::agent::semantic_validate;
use crate::agent::telemetry;
use crate::agent::validate::ErrorCategory;
use crate::ai::cost;
use crate::ai::message::ChatMessage;
use crate::ai::provider::{StreamDelta, TokenUsage};
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
    RetrievalStatus {
        message: String,
        items: Vec<crate::agent::retrieval::RetrievedContextItem>,
        used_embeddings: bool,
        lexical_fallback: bool,
    },
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
        fatal_combo: bool,
        negation_conflict: bool,
        repair_sensitive_ops: Vec<String>,
    },
    /// Generation confidence assessment based on plan risk + cookbook matching.
    ConfidenceAssessment {
        level: String,
        score: u32,
        cookbook_matches: Vec<String>,
        warnings: Vec<String>,
        message: String,
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
    PartCodeExtracted {
        part_index: usize,
        part_name: String,
        code: String,
    },
    PartStlReady {
        part_index: usize,
        part_name: String,
        stl_base64: String,
    },
    PartStlFailed {
        part_index: usize,
        part_name: String,
        error: String,
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
    StaticValidationReport {
        passed: bool,
        findings: Vec<String>,
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
    PostGeometryValidationReport {
        report: executor::PostGeometryValidationReport,
    },
    PostGeometryValidationWarning {
        message: String,
    },
    SemanticValidationReport {
        part_name: String,
        passed: bool,
        findings: Vec<String>,
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
    /// Prompt triage determined the request needs clarifying questions.
    ClarificationNeeded {
        questions: Vec<String>,
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
    pub clarification_questions: Option<Vec<String>>,
}

/// Outcome from the generation pipeline, used for session memory recording.
struct PipelineOutcome {
    response: String,
    final_code: Option<String>,
    success: bool,
    error: Option<String>,
    validation_attempts: Option<u32>,
    static_findings: Vec<String>,
    post_check_soft_failed: bool,
    post_check_soft_fail_reason: Option<String>,
    part_acceptance_rate: Option<f32>,
    assembly_success_rate: Option<f32>,
    partial_preview_shown: bool,
    empty_viewport_after_generation: bool,
    retry_ladder_stage_reached: Option<u32>,
    failure_signatures: Vec<String>,
}

/// Record a generation attempt into the session memory.
fn record_generation_attempt(
    state: &AppState,
    user_request: &str,
    code: Option<&str>,
    success: bool,
    error_category: Option<ErrorCategory>,
    failing_operation: Option<String>,
    error_summary: Option<String>,
) {
    let operations = code
        .map(memory::extract_operations_from_code)
        .unwrap_or_default();
    let attempt = memory::GenerationAttempt {
        user_request: user_request.chars().take(80).collect(),
        operations_used: operations,
        success,
        error_category,
        failing_operation,
        error_summary: error_summary.map(|s| s.chars().take(120).collect()),
    };
    state.session_memory.lock().unwrap().record_attempt(attempt);
}

fn record_generation_trace(
    config: &crate::config::AppConfig,
    user_request: &str,
    retrieval_result: &retrieval::RetrievalResult,
    plan_risk_score: Option<u32>,
    outcome: &PipelineOutcome,
) {
    if !config.telemetry_enabled {
        return;
    }

    let semantic_failure_signatures = outcome
        .failure_signatures
        .iter()
        .filter(|s| s.to_lowercase().contains("semantic"))
        .cloned()
        .collect::<Vec<_>>();
    let split_part_rejection_count = outcome
        .failure_signatures
        .iter()
        .filter(|s| {
            let lower = s.to_lowercase();
            lower.contains("component count") || lower.contains("split_part")
        })
        .count() as u32;
    let multipart_contract_failure_count = outcome
        .failure_signatures
        .iter()
        .filter(|s| {
            let lower = s.to_lowercase();
            lower.contains("multipart contract")
                || lower.contains("assembly contract")
                || lower.contains("required multipart")
        })
        .count() as u32;
    let fallback_activation_count = outcome
        .failure_signatures
        .iter()
        .filter(|s| s.starts_with("fallback_activated:"))
        .count() as u32;
    let fallback_activation_rate = if let Some(acceptance_rate) = outcome.part_acceptance_rate {
        let denom = if acceptance_rate > 0.0 {
            (1.0 / acceptance_rate).max(1.0)
        } else {
            1.0
        };
        Some((fallback_activation_count as f32 / denom).min(1.0))
    } else {
        Some(if fallback_activation_count > 0 {
            1.0
        } else {
            0.0
        })
    };
    let false_success_count = if outcome.success
        && (multipart_contract_failure_count > 0
            || outcome
                .part_acceptance_rate
                .map(|r| r < 1.0)
                .unwrap_or(false))
    {
        1
    } else {
        0
    };

    let trace = telemetry::GenerationTraceV1 {
        version: 1,
        timestamp_ms: telemetry::now_ms(),
        request_hash: telemetry::hash_request(user_request),
        intent_tags: telemetry::infer_intent_tags(user_request),
        provider: config.ai_provider.clone(),
        model: config.model.clone(),
        retrieved_items: retrieval_result
            .items
            .iter()
            .map(|i| telemetry::TraceRetrievedItem {
                source: i.source.clone(),
                id: i.id.clone(),
                title: i.title.clone(),
                score: i.score,
            })
            .collect(),
        plan_risk_score,
        confidence_score: None,
        static_findings: outcome.static_findings.clone(),
        execution_success: outcome.success,
        retry_attempts: outcome.validation_attempts,
        final_error: outcome.error.clone(),
        post_check_soft_failed: outcome.post_check_soft_failed,
        post_check_soft_fail_reason: outcome.post_check_soft_fail_reason.clone(),
        part_acceptance_rate: outcome.part_acceptance_rate,
        assembly_success_rate: outcome.assembly_success_rate,
        semantic_acceptance_rate: outcome.part_acceptance_rate,
        fallback_activation_count,
        multipart_contract_failure_count,
        false_success_count,
        false_fatal_plan_rejection_count: 0,
        fallback_activation_rate,
        split_part_rejection_count,
        semantic_failure_signatures,
        partial_preview_shown: outcome.partial_preview_shown,
        empty_viewport_after_generation: outcome.empty_viewport_after_generation,
        retry_ladder_stage_reached: outcome.retry_ladder_stage_reached,
        failure_signatures: outcome.failure_signatures.clone(),
        mechanism_candidates: retrieval_result
            .items
            .iter()
            .filter(|i| i.source == "mechanism")
            .map(|i| i.title.clone())
            .collect(),
        mechanism_selected_ids: retrieval_result
            .items
            .iter()
            .filter(|i| i.source == "mechanism")
            .map(|i| i.id.clone())
            .collect(),
    };

    if let Err(e) = telemetry::write_trace(&trace) {
        eprintln!("telemetry write failed: {}", e);
    }
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
      "description": "Compact geometric description (max ~280 chars) in mm. State robust primary path + optional high-fidelity fallback path. Keep it concise and self-contained.",
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
- Include all dimensions in mm
- Include geometric detail from the design plan: profiles, cross-sections, radii
- Use reliability-first phrasing for the primary path (prefer extrude + cut/union + explicit intermediates)
- Mention shell+loft only as optional fallback when absolutely required
- Keep each part `description` short (single paragraph, <= 280 chars) to avoid truncation.
- Each part description must be self-contained (another AI must be able to build it without other context)

Rules:
- Part names must be valid Python identifiers (snake_case)
- Positions are in mm, relative to origin [0,0,0]
- Do NOT decompose decorative features, fillets, or chamfers into separate parts

Keep your response as short as possible. For single mode, return ONLY {"mode":"single"} with no other text."#;

fn reliability_policy_text(profile: &crate::config::GenerationReliabilityProfile) -> &'static str {
    match profile {
        crate::config::GenerationReliabilityProfile::ReliabilityFirst => {
            "reliability_first: avoid loft+shell first-pass, avoid blanket edges().fillet(), prefer robust primitives + boolean subtraction."
        }
        crate::config::GenerationReliabilityProfile::Balanced => {
            "balanced: use robust operations first, allow advanced operations if strongly justified."
        }
        crate::config::GenerationReliabilityProfile::FidelityFirst => {
            "fidelity_first: prioritize geometric fidelity but still provide safe fallback in comments/structure."
        }
    }
}

fn build_part_prompt(
    system_prompt: &str,
    part: &PartSpec,
    design_context: &str,
    config: &crate::config::AppConfig,
) -> String {
    format!(
        "{}\n\n\
        ## Geometry Design Context\n{}\n\n\
        ## IMPORTANT: You are generating ONE SPECIFIC PART of a multi-part assembly.\n\n\
        Generate ONLY this part: **{}**\n\n\
        Description: {}\n\n\
        Constraints:\n{}\n\n\
        Active reliability policy: {}\n\
        Max operation budget: keep the script under ~22 geometric operations before optional polish.\n\
        The final result variable MUST contain ONLY this single part.\n\
        Do NOT generate any other parts. Do NOT create an assembly.\n\n\
        ## CadQuery Construction Rules (MANDATORY)\n\
        - ALWAYS use `centered=(True, True, False)` so the base sits at Z=0\n\
        - NEVER use shell() — subtract an explicit inner solid instead\n\
        - Wrap ALL fillet/chamfer/loft in try/except with graceful fallback\n\
        - union() calls MUST have volumetric overlap (0.2mm+), not just face-touching\n\
        - Result MUST be a single connected solid\n\
        - Use rect().extrude() + edges(\"|Z\").fillet(R) for rounded rectangles\n\
        - For hollow frames (lips, ridges): build as outer.cut(inner), overlap with base before union\n\
        - Use named intermediates, not one giant chain\n\n\
        STRICT OUTPUT CONTRACT:\n\
        - Return code only (no prose).\n\
        - Wrap code in <CODE>...</CODE> tags.\n\
        - Must assign final geometry to variable `result`.\n\
        - Keep repair-friendly structure (named intermediates over one giant chain).",
        system_prompt,
        design_context,
        part.name,
        part.description,
        part.constraints
            .iter()
            .map(|c| format!("- {}", c))
            .collect::<Vec<_>>()
            .join("\n"),
        reliability_policy_text(&config.generation_reliability_profile),
    )
}

async fn request_code_only_part_retry(
    config: &crate::config::AppConfig,
    system_prompt: &str,
    part: &PartSpec,
    design_context: &str,
    previous_response: &str,
) -> Result<(Option<String>, Option<TokenUsage>), AppError> {
    let provider = create_provider(config)?;
    let strict_prompt = format!(
        "{}\n\n\
        STRICT OUTPUT RULES (MANDATORY):\n\
        - Return ONLY executable Python CadQuery code.\n\
        - Wrap code in <CODE>...</CODE> tags.\n\
        - No prose, no markdown explanation, no bullets.\n\
        - Must assign final geometry to variable named result.\n\
        - Generate ONLY the '{}' part.",
        build_part_prompt(system_prompt, part, design_context, config),
        part.name
    );

    let retry_user = format!(
        "Previous response was non-code and could not be executed.\n\
        Return the CadQuery code now.\n\n\
        Part: {}\n\
        Description: {}\n\
        Constraints:\n{}\n\n\
        Previous invalid response (for context):\n{}",
        part.name,
        part.description,
        part.constraints
            .iter()
            .map(|c| format!("- {}", c))
            .collect::<Vec<_>>()
            .join("\n"),
        previous_response.chars().take(2000).collect::<String>()
    );

    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: strict_prompt,
        },
        ChatMessage {
            role: "user".to_string(),
            content: retry_user,
        },
    ];

    let (response, usage) = provider.complete(&messages, None).await?;
    Ok((extract_code_from_response(&response), usage))
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
                !trimmed.starts_with("import cadquery") && !trimmed.starts_with("from cadquery")
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

fn assembly_contract_issues(code: &str, parts: &[(String, String, [f64; 3])]) -> Vec<String> {
    let mut issues = Vec::new();
    for (name, _code, _pos) in parts {
        let var_name = format!("part_{}", name);
        if !code.contains(&var_name) {
            issues.push(format!("missing {}", var_name));
        }
        let add_call = format!("assy.add({},", var_name);
        if !code.contains(&add_call) {
            issues.push(format!("missing assy.add for {}", var_name));
        }
    }

    if !code.contains("assy = cq.Assembly()") {
        issues.push("missing assembly initialization".to_string());
    }
    if !code.contains("result = assy.toCompound()") {
        issues.push("missing assembly compound result".to_string());
    }

    issues
}

fn format_bbox_hint_from_dims(dims: [f64; 3]) -> String {
    format!(
        "overall envelope {:.3}x{:.3}x{:.3}mm",
        dims[0], dims[1], dims[2]
    )
}

fn build_part_bbox_hint(
    semantic_contract: Option<&semantic_validate::SemanticPartContract>,
    part_request: &str,
    mode: &crate::config::SemanticBboxMode,
) -> Option<String> {
    match mode {
        crate::config::SemanticBboxMode::Legacy => Some(part_request.to_string()),
        crate::config::SemanticBboxMode::SemanticAware => {
            if let Some(contract) = semantic_contract {
                if let Some(expected) = &contract.expected_bbox_mm {
                    return Some(format_bbox_hint_from_dims(expected.sorted_extents_mm));
                }
            }
            semantic_validate::infer_envelope_dimensions_mm(part_request)
                .map(format_bbox_hint_from_dims)
        }
    }
}

fn build_assembly_bbox_hint(
    plan: &GenerationPlan,
    user_request: &str,
    mode: &crate::config::SemanticBboxMode,
) -> Option<String> {
    match mode {
        crate::config::SemanticBboxMode::Legacy => Some(user_request.to_string()),
        crate::config::SemanticBboxMode::SemanticAware => {
            let mut aggregate: Option<[f64; 3]> = None;
            for part in &plan.parts {
                let contract =
                    semantic_validate::build_default_contract(&part.name, &part.description);
                if let Some(expected) = contract.expected_bbox_mm {
                    aggregate = Some(match aggregate {
                        Some(current) => [
                            current[0].max(expected.sorted_extents_mm[0]),
                            current[1].max(expected.sorted_extents_mm[1]),
                            current[2].max(expected.sorted_extents_mm[2]),
                        ],
                        None => expected.sorted_extents_mm,
                    });
                }
            }

            if let Some(dims) = aggregate {
                Some(format_bbox_hint_from_dims(dims))
            } else {
                semantic_validate::infer_envelope_dimensions_mm(user_request)
                    .map(format_bbox_hint_from_dims)
            }
        }
    }
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

fn forward_validation_event(on_event: &Channel<MultiPartEvent>, evt: executor::ValidationEvent) {
    match evt {
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
        executor::ValidationEvent::StaticValidation { passed, findings } => {
            let _ = on_event.send(MultiPartEvent::StaticValidationReport { passed, findings });
        }
        executor::ValidationEvent::Success { attempt, message } => {
            let _ = on_event.send(MultiPartEvent::ValidationSuccess { attempt, message });
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
        executor::ValidationEvent::PostGeometryValidation { report } => {
            let _ = on_event.send(MultiPartEvent::PostGeometryValidationReport { report });
        }
        executor::ValidationEvent::PostGeometryWarning { message } => {
            let _ = on_event.send(MultiPartEvent::PostGeometryValidationWarning { message });
        }
    }
}

async fn build_system_prompt_with_retrieval(
    config: &crate::config::AppConfig,
    cq_version: Option<&str>,
    query: &str,
    session_context: Option<String>,
    on_event: &Channel<MultiPartEvent>,
) -> (String, retrieval::RetrievalResult) {
    let base = prompts::build_compact_system_prompt_for_preset(
        config.agent_rules_preset.as_deref(),
        cq_version,
    );

    let _ = on_event.send(MultiPartEvent::RetrievalStatus {
        message: "Retrieving CAD guidance...".to_string(),
        items: vec![],
        used_embeddings: false,
        lexical_fallback: false,
    });

    let mut retrieval_result = retrieval::retrieve_context(
        query,
        config,
        config.agent_rules_preset.as_deref(),
        cq_version,
    )
    .await;

    let _ = on_event.send(MultiPartEvent::RetrievalStatus {
        message: if retrieval_result.items.is_empty() {
            "No retrieval snippets matched; using compact core prompt.".to_string()
        } else {
            format!(
                "Selected {} retrieval snippets.",
                retrieval_result.items.len()
            )
        },
        items: retrieval_result.items.clone(),
        used_embeddings: retrieval_result.used_embeddings,
        lexical_fallback: retrieval_result.lexical_fallback,
    });

    let mut system_prompt = base;
    if let Some(ctx) = session_context {
        system_prompt.push_str("\n\n");
        system_prompt.push_str(&ctx);
    }
    if !retrieval_result.context_markdown.is_empty() {
        system_prompt.push_str("\n\n");
        system_prompt.push_str(&retrieval_result.context_markdown);
    }

    if retrieval_result.items.is_empty() {
        retrieval_result = retrieval::RetrievalResult::empty();
    }

    (system_prompt, retrieval_result)
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
    state: &AppState,
) -> Result<(design::DesignPlan, DesignPlanResult), AppError> {
    let _ = on_event.send(MultiPartEvent::PlanStatus {
        message: "Designing geometry...".to_string(),
    });

    let design_extra_context = {
        let rules =
            crate::agent::rules::AgentRules::from_preset(config.agent_rules_preset.as_deref()).ok();
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
        // Append session memory context so the geometry advisor knows what failed
        if let Some(session_ctx) = state.session_memory.lock().unwrap().build_context_section() {
            if !ctx.is_empty() {
                ctx.push_str("\n\n");
            }
            ctx.push_str(&session_ctx);
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

    // Guard: if the AI returned empty text, return a clear error instead of
    // silently validating an empty plan (which produces misleading 7/10 risk).
    if design_plan.text.trim().is_empty() {
        return Err(AppError::AiProviderError(
            "AI returned an empty design plan. Check your API key, model name, and provider settings.".to_string(),
        ));
    }

    let mut validation = design::validate_plan_with_profile(
        &design_plan.text,
        &config.generation_reliability_profile,
    );

    let _ = on_event.send(MultiPartEvent::PlanValidation {
        risk_score: validation.risk_score,
        warnings: validation.warnings.clone(),
        is_valid: validation.is_valid,
        rejected_reason: validation.rejected_reason.clone(),
        fatal_combo: validation.risk_signals.fatal_combo,
        negation_conflict: validation.risk_signals.negation_conflict,
        repair_sensitive_ops: validation.risk_signals.repair_sensitive_ops.clone(),
    });

    // Give the planner multiple chances to return a valid structured plan.
    // This significantly reduces failures from malformed first responses.
    const MAX_PLAN_ATTEMPTS: usize = 3;
    let mut attempts = 1usize;
    while !validation.is_valid && attempts < MAX_PLAN_ATTEMPTS {
        let _ = on_event.send(MultiPartEvent::PlanStatus {
            message: format!(
                "Design plan too risky (score {}/10), re-planning (attempt {}/{})...",
                validation.risk_score,
                attempts + 1,
                MAX_PLAN_ATTEMPTS
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

        validation = design::validate_plan_with_profile(
            &design_plan.text,
            &config.generation_reliability_profile,
        );
        let _ = on_event.send(MultiPartEvent::PlanValidation {
            risk_score: validation.risk_score,
            warnings: validation.warnings.clone(),
            is_valid: validation.is_valid,
            rejected_reason: validation.rejected_reason.clone(),
            fatal_combo: validation.risk_signals.fatal_combo,
            negation_conflict: validation.risk_signals.negation_conflict,
            repair_sensitive_ops: validation.risk_signals.repair_sensitive_ops.clone(),
        });

        attempts += 1;
    }

    let final_risk_score = validation.risk_score;
    let final_warnings = validation.warnings.clone();
    let final_is_valid = validation.is_valid;

    let _ = on_event.send(MultiPartEvent::DesignPlan {
        plan_text: design_plan.text.clone(),
    });

    // Compute and emit confidence assessment
    {
        let confidence_rules =
            crate::agent::rules::AgentRules::from_preset(config.agent_rules_preset.as_deref()).ok();
        let cookbook_ref = confidence_rules
            .as_ref()
            .and_then(|r| r.cookbook.as_deref());
        let patterns_ref = confidence_rules
            .as_ref()
            .and_then(|r| r.design_patterns.as_deref());

        let conf = confidence::assess_confidence_with_profile(
            &validation,
            cookbook_ref,
            patterns_ref,
            &config.generation_reliability_profile,
        );
        let _ = on_event.send(MultiPartEvent::ConfidenceAssessment {
            level: match conf.level {
                confidence::ConfidenceLevel::High => "high".to_string(),
                confidence::ConfidenceLevel::Medium => "medium".to_string(),
                confidence::ConfidenceLevel::Low => "low".to_string(),
            },
            score: conf.score,
            cookbook_matches: conf
                .cookbook_matches
                .iter()
                .map(|m| m.title.clone())
                .collect(),
            warnings: conf.warnings.clone(),
            message: conf.message.clone(),
        });
    }

    let result = DesignPlanResult {
        plan_text: design_plan.text.clone(),
        risk_score: final_risk_score,
        warnings: final_warnings,
        is_valid: final_is_valid,
        clarification_questions: None,
    };

    Ok((design_plan, result))
}

/// Phase 1+: Planner decomposition, code generation (single/multi/iterative/consensus),
/// review, and validation. Returns a `PipelineOutcome` for session memory recording.
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
) -> Result<PipelineOutcome, AppError> {
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

    let requires_multipart_contract = request_requires_multipart_contract(user_request, plan_text);
    let planner_system = PLANNER_SYSTEM_PROMPT.to_string();
    let mut plan: Option<GenerationPlan> = None;
    let mut last_parse_err: Option<String> = None;
    let mut planner_response = String::new();
    let mut planner_parse_failures: u32 = 0;
    const MAX_PLANNER_PARSE_ATTEMPTS: usize = 3;
    const PLANNER_MAX_TOKENS: u32 = 3072;

    for attempt in 1..=MAX_PLANNER_PARSE_ATTEMPTS {
        let planner = create_provider(config)?;
        let planner_messages = if attempt == 1 {
            vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: planner_system.clone(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: enhanced_message.clone(),
                },
            ]
        } else {
            let retry_instruction = format!(
                "Your previous response could not be parsed as JSON ({:?}). Return ONLY valid compact JSON now. \
                 Keep each part description <= 280 chars. No markdown, no prose.\n\nPrevious response:\n{}",
                last_parse_err,
                planner_response.chars().take(2500).collect::<String>()
            );
            vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: planner_system.clone(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: enhanced_message.clone(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: retry_instruction,
                },
            ]
        };

        let (plan_json, plan_usage) = planner
            .complete(&planner_messages, Some(PLANNER_MAX_TOKENS))
            .await?;
        planner_response = plan_json.clone();
        if let Some(ref u) = plan_usage {
            total_usage.add(u);
            emit_usage(on_event, "plan", u, provider_id, model_id);
        }

        match parse_plan(&plan_json) {
            Ok(parsed) => {
                plan = Some(parsed);
                break;
            }
            Err(parse_err) => {
                last_parse_err = Some(parse_err.clone());
                planner_parse_failures = planner_parse_failures.saturating_add(1);
                if attempt < MAX_PLANNER_PARSE_ATTEMPTS {
                    let _ = on_event.send(MultiPartEvent::PlanStatus {
                        message: format!(
                            "Planner JSON parse failed (attempt {}/{}), retrying with strict compact JSON...",
                            attempt, MAX_PLANNER_PARSE_ATTEMPTS
                        ),
                    });
                }
            }
        }
    }

    let plan: GenerationPlan = match plan {
        Some(p) => p,
        None => {
            if requires_multipart_contract {
                let parse_err = last_parse_err
                    .clone()
                    .unwrap_or_else(|| "unknown planner parse error".to_string());
                return Err(AppError::AiProviderError(format!(
                    "Planner failed to decompose parts after {} attempt(s): {}",
                    planner_parse_failures, parse_err
                )));
            } else {
                let _ = on_event.send(MultiPartEvent::PlanStatus {
                    message:
                        "Planner returned invalid JSON; falling back to single-part generation."
                            .to_string(),
                });
                GenerationPlan {
                    mode: "single".to_string(),
                    description: None,
                    parts: vec![],
                }
            }
        }
    };

    if requires_multipart_contract && (plan.mode != "multi" || plan.parts.len() < 2) {
        return Err(AppError::AiProviderError(
            "Planner failed to produce a valid multipart decomposition — the plan did not contain at least 2 parts.".to_string(),
        ));
    }
    let _ = on_event.send(MultiPartEvent::PlanResult { plan: plan.clone() });

    // -----------------------------------------------------------------------
    // Single mode: fall through to normal streaming
    // -----------------------------------------------------------------------
    if plan.mode == "single" || plan.parts.is_empty() {
        // Check if iterative mode should be used
        let build_steps = iterative::parse_build_steps(plan_text);

        if iterative::should_use_iterative(&build_steps) {
            if let Some(ctx) = execution_ctx {
                let _ = on_event.send(MultiPartEvent::PlanStatus {
                    message: format!("Building step by step ({} steps)...", build_steps.len()),
                });

                let on_iter_event = |evt: iterative::IterativeEvent| match evt {
                    iterative::IterativeEvent::Start { total_steps, steps } => {
                        let _ =
                            on_event.send(MultiPartEvent::IterativeStart { total_steps, steps });
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

                let iter_error = if result.success {
                    None
                } else {
                    Some("Iterative build failed".to_string())
                };
                let _ = on_event.send(MultiPartEvent::Done {
                    success: result.success,
                    error: iter_error.clone(),
                    validated: true,
                });

                return Ok(PipelineOutcome {
                    response: result.final_code.clone(),
                    final_code: Some(result.final_code),
                    success: result.success,
                    error: iter_error,
                    validation_attempts: None,
                    static_findings: vec![],
                    post_check_soft_failed: false,
                    post_check_soft_fail_reason: None,
                    part_acceptance_rate: None,
                    assembly_success_rate: None,
                    partial_preview_shown: result.stl_base64.is_some(),
                    empty_viewport_after_generation: result.stl_base64.is_none(),
                    retry_ladder_stage_reached: None,
                    failure_signatures: vec![],
                });
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
                        let _ = on_event.send(MultiPartEvent::ConsensusStarted { candidate_count });
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
                            &config.reviewer_mode,
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
                        let on_validation_event = |evt: executor::ValidationEvent| {
                            forward_validation_event(on_event, evt)
                        };

                        let validation_result = executor::validate_and_retry(
                            final_code.clone(),
                            ctx,
                            system_prompt,
                            Some(user_request),
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

                    return Ok(PipelineOutcome {
                        response: response_text,
                        final_code: Some(final_code),
                        success: true,
                        error: None,
                        validation_attempts: None,
                        static_findings: vec![],
                        post_check_soft_failed: false,
                        post_check_soft_fail_reason: None,
                        part_acceptance_rate: None,
                        assembly_success_rate: None,
                        partial_preview_shown: false,
                        empty_viewport_after_generation: false,
                        retry_ladder_stage_reached: None,
                        failure_signatures: vec![],
                    });
                }

                // Consensus failed — fall through
                let _ = on_event.send(MultiPartEvent::PlanStatus {
                    message:
                        "Consensus failed to produce code, falling back to single generation..."
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
                match review::review_code(
                    review_provider,
                    user_request,
                    code,
                    Some(plan_text),
                    &config.reviewer_mode,
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
            let on_validation_event =
                |evt: executor::ValidationEvent| forward_validation_event(on_event, evt);

            let validation_result = executor::validate_and_retry(
                code.clone(),
                ctx,
                system_prompt,
                Some(user_request),
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
                error: validation_result.error.clone(),
                validated: true,
            });

            return Ok(PipelineOutcome {
                response: final_response,
                final_code: Some(validation_result.code),
                success: validation_result.success,
                error: validation_result.error,
                validation_attempts: Some(validation_result.attempts),
                static_findings: validation_result.static_findings,
                post_check_soft_failed: validation_result.post_check_warning.is_some(),
                post_check_soft_fail_reason: validation_result.post_check_warning,
                part_acceptance_rate: None,
                assembly_success_rate: None,
                partial_preview_shown: validation_result.stl_base64.is_some(),
                empty_viewport_after_generation: validation_result.stl_base64.is_none(),
                retry_ladder_stage_reached: validation_result.retry_ladder_stage_reached,
                failure_signatures: vec![],
            });
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

        // Guard: report failure if no code was extracted from the AI response
        let has_code = final_code.is_some();
        let no_code_error = if has_code {
            None
        } else {
            Some("No code block extracted from AI response".to_string())
        };

        let _ = on_event.send(MultiPartEvent::Done {
            success: has_code,
            error: no_code_error.clone(),
            validated: false,
        });

        return Ok(PipelineOutcome {
            response: final_response,
            final_code,
            success: has_code,
            error: no_code_error,
            validation_attempts: None,
            static_findings: vec![],
            post_check_soft_failed: false,
            post_check_soft_fail_reason: None,
            part_acceptance_rate: None,
            assembly_success_rate: None,
            partial_preview_shown: false,
            empty_viewport_after_generation: !has_code,
            retry_ladder_stage_reached: None,
            failure_signatures: if has_code {
                vec![]
            } else {
                vec!["no_code_extracted".to_string()]
            },
        });
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
        let part_prompt = build_part_prompt(system_prompt, part, plan_text, config);
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
        let part_spec = plan.parts[idx].clone();

        match handle.await {
            Ok((_, Ok((response, part_usage)))) => {
                if let Some(ref u) = part_usage {
                    total_usage.add(u);
                }
                let mut code = extract_code_from_response(&response);
                if code.is_none() {
                    let _ = on_event.send(MultiPartEvent::PlanStatus {
                        message: format!(
                            "Part '{}' returned non-code output. Requesting strict code-only retry...",
                            part_spec.name
                        ),
                    });
                    match request_code_only_part_retry(
                        config,
                        &system_prompt,
                        &part_spec,
                        plan_text,
                        &response,
                    )
                    .await
                    {
                        Ok((retried, usage)) => {
                            if let Some(ref u) = usage {
                                total_usage.add(u);
                            }
                            code = retried;
                        }
                        Err(e) => {
                            let _ = on_event.send(MultiPartEvent::PartComplete {
                                part_index: idx,
                                part_name: name,
                                success: false,
                                error: Some(format!("Code-only retry failed: {}", e)),
                            });
                            continue;
                        }
                    }
                }
                match code {
                    Some(c) => {
                        // Emit PartCodeExtracted before PartComplete
                        let _ = on_event.send(MultiPartEvent::PartCodeExtracted {
                            part_index: idx,
                            part_name: name.clone(),
                            code: c.clone(),
                        });
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

    // Per-part acceptance before assembly (static validate + execute/repair + geometry checks).
    let mut accepted_parts: Vec<(String, String, [f64; 3])> = Vec::new();
    let mut accepted_retry_stage: Option<u32> = None;
    let mut part_failure_signatures: Vec<String> = Vec::new();
    let mut partial_preview_available = false;

    if let Some(ctx) = execution_ctx {
        for (part_idx, part_entry) in part_codes.iter_mut().enumerate() {
            if let Some((name, code, pos)) = part_entry.clone() {
                let part_request = plan.parts[part_idx].description.clone();
                let semantic_contract =
                    semantic_validate::build_default_contract(&name, &part_request);
                let preview_ctx = executor::ExecutionContext {
                    venv_dir: ctx.venv_dir.clone(),
                    runner_script: ctx.runner_script.clone(),
                    config: config.clone(),
                };

                let artifact_result = evaluate_part_acceptance(
                    &code,
                    &preview_ctx,
                    system_prompt,
                    &part_request,
                    &name,
                    Some(&semantic_contract),
                )
                .await;

                match artifact_result {
                    Ok(artifact) => {
                        let _ = on_event.send(MultiPartEvent::SemanticValidationReport {
                            part_name: name.clone(),
                            passed: true,
                            findings: artifact.semantic_findings.clone(),
                        });
                        if let Some(stage) = artifact.retry_ladder_stage_reached {
                            accepted_retry_stage =
                                Some(accepted_retry_stage.map(|s| s.max(stage)).unwrap_or(stage));
                        }
                        if let Some(ref report) = artifact.post_geometry_report {
                            let _ = on_event.send(MultiPartEvent::PostGeometryValidationReport {
                                report: report.clone(),
                            });
                        }
                        if let Some(ref warning) = artifact.post_check_warning {
                            let _ = on_event.send(MultiPartEvent::PostGeometryValidationWarning {
                                message: warning.clone(),
                            });
                        }
                        if config.preview_on_partial_failure {
                            if let Some(stl_base64) = artifact.stl_base64.clone() {
                                partial_preview_available = true;
                                let _ = on_event.send(MultiPartEvent::PartStlReady {
                                    part_index: part_idx,
                                    part_name: name.clone(),
                                    stl_base64,
                                });
                            }
                        }
                        *part_entry = Some((name.clone(), artifact.code.clone(), pos));
                        accepted_parts.push((name, artifact.code, pos));
                    }
                    Err(e) => {
                        let semantic_findings = if e.contains("semantic validation failed: ") {
                            e.trim_start_matches("semantic validation failed: ")
                                .split(';')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect::<Vec<_>>()
                        } else {
                            vec![e.clone()]
                        };
                        let _ = on_event.send(MultiPartEvent::SemanticValidationReport {
                            part_name: name.clone(),
                            passed: false,
                            findings: semantic_findings,
                        });
                        part_failure_signatures.push(e.clone());
                        *part_entry = None;
                        let _ = on_event.send(MultiPartEvent::PartStlFailed {
                            part_index: part_idx,
                            part_name: name.clone(),
                            error: e.clone(),
                        });
                        let _ = on_event.send(MultiPartEvent::PartComplete {
                            part_index: part_idx,
                            part_name: name,
                            success: false,
                            error: Some(format!("Rejected in per-part acceptance: {}", e)),
                        });
                    }
                }
            }
        }
    } else {
        accepted_parts = part_codes.into_iter().flatten().collect();
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

    if accepted_parts.is_empty() {
        part_failure_signatures.push("semantic_acceptance_all_rejected".to_string());
        let _ = on_event.send(MultiPartEvent::Done {
            success: false,
            error: Some("All generated parts were rejected by per-part acceptance".to_string()),
            validated: true,
        });
        return Ok(PipelineOutcome {
            response: String::new(),
            final_code: None,
            success: false,
            error: Some("All generated parts were rejected by per-part acceptance".to_string()),
            validation_attempts: None,
            static_findings: vec![],
            post_check_soft_failed: false,
            post_check_soft_fail_reason: None,
            part_acceptance_rate: Some(0.0),
            assembly_success_rate: Some(0.0),
            partial_preview_shown: partial_preview_available,
            empty_viewport_after_generation: !partial_preview_available,
            retry_ladder_stage_reached: accepted_retry_stage,
            failure_signatures: part_failure_signatures,
        });
    }

    // -----------------------------------------------------------------------
    // Phase 3: Assemble
    // -----------------------------------------------------------------------
    let _ = on_event.send(MultiPartEvent::AssemblyStatus {
        message: "Assembling parts...".to_string(),
    });

    let successful_parts = accepted_parts;
    let strict_multipart_required =
        config.quality_gates_strict && request_requires_multipart_contract(user_request, plan_text);
    let required_parts_met =
        !strict_multipart_required || successful_parts.len() == plan.parts.len();

    match assemble_parts(&successful_parts) {
        Ok(code) => {
            // Emit assembled code early — if the pipeline times out during
            // review/validation, the frontend still has usable code.
            let _ = on_event.send(MultiPartEvent::FinalCode {
                code: code.clone(),
                stl_base64: None,
            });

            let final_code = if config.enable_code_review {
                let _ = on_event.send(MultiPartEvent::ReviewStatus {
                    message: "Reviewing assembled code...".to_string(),
                });
                let review_provider = create_provider(config)?;
                match review::review_code(
                    review_provider,
                    user_request,
                    &code,
                    Some(plan_text),
                    &config.reviewer_mode,
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
                            let review_issues =
                                assembly_contract_issues(&result.code, &successful_parts);
                            if review_issues.is_empty() {
                                result.code
                            } else {
                                let _ = on_event.send(MultiPartEvent::PlanStatus {
                                    message: format!(
                                        "Reviewer output dropped multipart structure ({}). Keeping assembled code.",
                                        review_issues.join(", ")
                                    ),
                                });
                                code
                            }
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
                let on_validation_event =
                    |evt: executor::ValidationEvent| forward_validation_event(on_event, evt);

                let assembly_bbox_hint =
                    build_assembly_bbox_hint(&plan, user_request, &config.semantic_bbox_mode);
                let validation_result = executor::validate_and_retry(
                    final_code.clone(),
                    ctx,
                    system_prompt,
                    assembly_bbox_hint.as_deref(),
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

                let contract_issues =
                    assembly_contract_issues(&validation_result.code, &successful_parts);
                if config.quality_gates_strict && !contract_issues.is_empty() {
                    let msg = format!(
                        "Validation retry produced code that breaks multipart assembly contract: {}",
                        contract_issues.join(", ")
                    );
                    let _ = on_event.send(MultiPartEvent::Done {
                        success: false,
                        error: Some(msg.clone()),
                        validated: true,
                    });
                    let mut failure_signatures = part_failure_signatures.clone();
                    failure_signatures.push("multipart_contract_validation_failure".to_string());
                    failure_signatures.push(msg.clone());
                    return Ok(PipelineOutcome {
                        response: validation_result.code.clone(),
                        final_code: Some(validation_result.code),
                        success: false,
                        error: Some(msg),
                        validation_attempts: Some(validation_result.attempts),
                        static_findings: validation_result.static_findings,
                        post_check_soft_failed: validation_result.post_check_warning.is_some(),
                        post_check_soft_fail_reason: validation_result.post_check_warning,
                        part_acceptance_rate: Some(
                            successful_parts.len() as f32 / plan.parts.len() as f32,
                        ),
                        assembly_success_rate: Some(0.0),
                        partial_preview_shown: partial_preview_available,
                        empty_viewport_after_generation: !partial_preview_available,
                        retry_ladder_stage_reached: validation_result
                            .retry_ladder_stage_reached
                            .or(accepted_retry_stage),
                        failure_signatures,
                    });
                } else if !contract_issues.is_empty() {
                    let _ = on_event.send(MultiPartEvent::ReviewStatus {
                        message: format!(
                            "Assembly contract issues detected (non-strict mode): {}",
                            contract_issues.join(", ")
                        ),
                    });
                }

                let mut done_error = validation_result.error.clone();
                let final_success = if required_parts_met {
                    validation_result.success
                } else {
                    done_error = Some(format!(
                        "Only {}/{} parts accepted; strict multipart contract requires all parts.",
                        successful_parts.len(),
                        plan.parts.len()
                    ));
                    false
                };
                if !required_parts_met {
                    part_failure_signatures.push("multipart_contract_missing_parts".to_string());
                }

                if total_usage.total() > 0 {
                    emit_usage(on_event, "total", total_usage, provider_id, model_id);
                }

                let _ = on_event.send(MultiPartEvent::Done {
                    success: final_success,
                    error: done_error.clone(),
                    validated: true,
                });

                return Ok(PipelineOutcome {
                    response: validation_result.code.clone(),
                    final_code: Some(validation_result.code),
                    success: final_success,
                    error: done_error,
                    validation_attempts: Some(validation_result.attempts),
                    static_findings: validation_result.static_findings,
                    post_check_soft_failed: validation_result.post_check_warning.is_some(),
                    post_check_soft_fail_reason: validation_result.post_check_warning,
                    part_acceptance_rate: Some(
                        successful_parts.len() as f32 / plan.parts.len() as f32,
                    ),
                    assembly_success_rate: Some(if final_success { 1.0 } else { 0.0 }),
                    partial_preview_shown: partial_preview_available,
                    empty_viewport_after_generation: validation_result.stl_base64.is_none()
                        && !partial_preview_available,
                    retry_ladder_stage_reached: validation_result
                        .retry_ladder_stage_reached
                        .or(accepted_retry_stage),
                    failure_signatures: part_failure_signatures,
                });
            }

            // No execution context — emit as-is
            if total_usage.total() > 0 {
                emit_usage(on_event, "total", total_usage, provider_id, model_id);
            }

            let _ = on_event.send(MultiPartEvent::FinalCode {
                code: final_code.clone(),
                stl_base64: None,
            });
            let done_error = if required_parts_met {
                None
            } else {
                part_failure_signatures.push("multipart_contract_missing_parts".to_string());
                Some(format!(
                    "Only {}/{} parts accepted; strict multipart contract requires all parts.",
                    successful_parts.len(),
                    plan.parts.len()
                ))
            };
            let _ = on_event.send(MultiPartEvent::Done {
                success: done_error.is_none(),
                error: done_error.clone(),
                validated: false,
            });
            Ok(PipelineOutcome {
                response: final_code.clone(),
                final_code: Some(final_code),
                success: done_error.is_none(),
                error: done_error,
                validation_attempts: None,
                static_findings: vec![],
                post_check_soft_failed: false,
                post_check_soft_fail_reason: None,
                part_acceptance_rate: Some(successful_parts.len() as f32 / plan.parts.len() as f32),
                assembly_success_rate: Some(if required_parts_met { 1.0 } else { 0.0 }),
                partial_preview_shown: partial_preview_available,
                empty_viewport_after_generation: !partial_preview_available,
                retry_ladder_stage_reached: accepted_retry_stage,
                failure_signatures: part_failure_signatures,
            })
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
    let cq_version = state.cadquery_version.lock().unwrap().clone();
    let user_request = message.clone();
    let session_ctx = state.session_memory.lock().unwrap().build_context_section();
    let (system_prompt, retrieval_result) = build_system_prompt_with_retrieval(
        &config,
        cq_version.as_deref(),
        &message,
        session_ctx,
        &on_event,
    )
    .await;

    let provider_id = config.ai_provider.clone();
    let model_id = config.model.clone();
    let mut total_usage = TokenUsage::default();

    // Resolve execution context for backend validation (None if Python not set up)
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

    // -----------------------------------------------------------------------
    // Modification branch: detect and handle code modifications (early return)
    // -----------------------------------------------------------------------
    let modification_intent =
        modify::detect_modification_intent(&message, existing_code.as_deref());

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
                match review::review_code(
                    review_provider,
                    &user_request,
                    code,
                    None,
                    &config.reviewer_mode,
                )
                .await
                {
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
            let on_validation_event =
                |evt: executor::ValidationEvent| forward_validation_event(&on_event, evt);

            let validation_result = executor::validate_and_retry(
                code.clone(),
                ctx,
                &system_prompt,
                Some(&user_request),
                &on_validation_event,
            )
            .await?;

            if validation_result.retry_usage.total() > 0 {
                total_usage.add(&validation_result.retry_usage);
                emit_usage(
                    &on_event,
                    "validation",
                    &validation_result.retry_usage,
                    &provider_id,
                    &model_id,
                );
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
                error: validation_result.error.clone(),
                validated: true,
            });

            let outcome = PipelineOutcome {
                response: final_response.clone(),
                final_code: Some(validation_result.code.clone()),
                success: validation_result.success,
                error: validation_result.error.clone(),
                validation_attempts: Some(validation_result.attempts),
                static_findings: validation_result.static_findings.clone(),
                post_check_soft_failed: validation_result.post_check_warning.is_some(),
                post_check_soft_fail_reason: validation_result.post_check_warning.clone(),
                part_acceptance_rate: None,
                assembly_success_rate: None,
                partial_preview_shown: validation_result.stl_base64.is_some(),
                empty_viewport_after_generation: validation_result.stl_base64.is_none(),
                retry_ladder_stage_reached: validation_result.retry_ladder_stage_reached,
                failure_signatures: vec![],
            };

            record_generation_attempt(
                &state,
                &user_request,
                Some(&validation_result.code),
                validation_result.success,
                None,
                None,
                validation_result.error.clone(),
            );
            record_generation_trace(&config, &user_request, &retrieval_result, None, &outcome);

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

        // Guard: report failure if no code was extracted from the modification response
        let has_code = final_code.is_some();
        let no_code_error = if has_code {
            None
        } else {
            Some("No code block extracted from modification response".to_string())
        };

        let _ = on_event.send(MultiPartEvent::Done {
            success: has_code,
            error: no_code_error.clone(),
            validated: false,
        });

        record_generation_attempt(
            &state,
            &user_request,
            final_code.as_deref(),
            has_code,
            None,
            None,
            no_code_error.clone(),
        );
        let outcome = PipelineOutcome {
            response: final_response.clone(),
            final_code: final_code.clone(),
            success: has_code,
            error: no_code_error,
            validation_attempts: None,
            static_findings: vec![],
            post_check_soft_failed: false,
            post_check_soft_fail_reason: None,
            part_acceptance_rate: None,
            assembly_success_rate: None,
            partial_preview_shown: false,
            empty_viewport_after_generation: !has_code,
            retry_ladder_stage_reached: None,
            failure_signatures: vec![],
        };
        record_generation_trace(&config, &user_request, &retrieval_result, None, &outcome);

        return Ok(final_response);
    }

    // -----------------------------------------------------------------------
    // Phase 0: Geometry Design Plan (always runs)
    // -----------------------------------------------------------------------
    let (design_plan, plan_result) = run_design_plan_phase(
        &message,
        &config,
        &on_event,
        &mut total_usage,
        &provider_id,
        &model_id,
        &state,
    )
    .await?;

    // -----------------------------------------------------------------------
    // Phase 1+: Generation pipeline (planner, code gen, review, validation)
    // -----------------------------------------------------------------------
    let generation_timeout = Duration::from_secs(config.max_generation_runtime_seconds as u64);
    let outcome = match timeout(
        generation_timeout,
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
        ),
    )
    .await
    {
        Ok(outcome) => outcome?,
        Err(_) => {
            let msg = format!(
                "Generation runtime exceeded {} seconds",
                config.max_generation_runtime_seconds
            );
            let _ = on_event.send(MultiPartEvent::Done {
                success: false,
                error: Some(msg.clone()),
                validated: false,
            });
            return Err(AppError::AiProviderError(msg));
        }
    };

    record_generation_attempt(
        &state,
        &user_request,
        outcome.final_code.as_deref(),
        outcome.success,
        None,
        None,
        outcome.error.clone(),
    );
    record_generation_trace(
        &config,
        &user_request,
        &retrieval_result,
        Some(plan_result.risk_score),
        &outcome,
    );

    Ok(outcome.response)
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

    // Fast prompt triage — ask clarifying questions if the request is vague
    let triage_provider = create_provider(&config)?;
    let analysis = design::analyze_prompt_clarity(triage_provider, &message).await?;

    if analysis.needs_clarification {
        let _ = on_event.send(MultiPartEvent::ClarificationNeeded {
            questions: analysis.questions.clone(),
        });
        return Ok(DesignPlanResult {
            plan_text: String::new(),
            risk_score: 0,
            warnings: vec![],
            is_valid: false,
            clarification_questions: Some(analysis.questions),
        });
    }

    // If triage returned an enriched prompt, use it for better plan quality
    let effective_message = analysis
        .enriched_prompt
        .as_deref()
        .unwrap_or(&message);

    let (_design_plan, plan_result) = run_design_plan_phase(
        effective_message,
        &config,
        &on_event,
        &mut total_usage,
        &provider_id,
        &model_id,
        &state,
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
    let cq_version = state.cadquery_version.lock().unwrap().clone();
    let session_ctx = state.session_memory.lock().unwrap().build_context_section();
    let retrieval_query = format!("{}\n\n{}", user_request, plan_text);
    let (system_prompt, retrieval_result) = build_system_prompt_with_retrieval(
        &config,
        cq_version.as_deref(),
        &retrieval_query,
        session_ctx,
        &on_event,
    )
    .await;
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

    let generation_timeout = Duration::from_secs(config.max_generation_runtime_seconds as u64);
    let outcome = match timeout(
        generation_timeout,
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
        ),
    )
    .await
    {
        Ok(outcome) => outcome?,
        Err(_) => {
            let msg = format!(
                "Generation runtime exceeded {} seconds",
                config.max_generation_runtime_seconds
            );
            let _ = on_event.send(MultiPartEvent::Done {
                success: false,
                error: Some(msg.clone()),
                validated: false,
            });
            return Err(AppError::AiProviderError(msg));
        }
    };

    record_generation_attempt(
        &state,
        &user_request,
        outcome.final_code.as_deref(),
        outcome.success,
        None,
        None,
        outcome.error.clone(),
    );
    record_generation_trace(&config, &user_request, &retrieval_result, None, &outcome);

    Ok(outcome.response)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Detect if the user request explicitly requires a strict multipart assembly outcome.
fn request_requires_multipart_contract(user_request: &str, plan_text: &str) -> bool {
    let text = format!("{}\n{}", user_request, plan_text).to_lowercase();

    const EXPLICIT_MULTI_HINTS: [&str; 16] = [
        "separate part",
        "separate parts",
        "separate component",
        "separate components",
        "separate body",
        "separate bodies",
        "separate piece",
        "separate pieces",
        "back plate",
        "backplate",
        "multi-part",
        "multipart",
        "exploded view",
        "exploded",
        "bakplate",
        "eksplodert",
    ];

    if EXPLICIT_MULTI_HINTS.iter().any(|hint| text.contains(hint)) {
        return true;
    }

    // Guard for prompts phrased as "separate X" without exact phrase matches.
    text.contains("separate")
        && (text.contains("part") || text.contains("component") || text.contains("piece"))
}

/// Parse the planner JSON response.
fn parse_plan(json_str: &str) -> Result<GenerationPlan, String> {
    fn try_repair_json_fragment(input: &str) -> Option<String> {
        let mut s = input.trim().to_string();
        if s.is_empty() {
            return None;
        }

        let mut quote_count = 0usize;
        let mut escaped = false;
        for ch in s.chars() {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '"' {
                quote_count += 1;
            }
        }
        if quote_count % 2 == 1 {
            s.push('"');
        }

        let open_brackets = s.chars().filter(|c| *c == '[').count();
        let close_brackets = s.chars().filter(|c| *c == ']').count();
        if open_brackets > close_brackets {
            s.push_str(&"]".repeat(open_brackets - close_brackets));
        }

        let open_braces = s.chars().filter(|c| *c == '{').count();
        let close_braces = s.chars().filter(|c| *c == '}').count();
        if open_braces > close_braces {
            s.push_str(&"}".repeat(open_braces - close_braces));
        }

        Some(s)
    }

    // Try to extract JSON from the response (the AI might wrap it in markdown fences)
    let cleaned = json_str
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    match serde_json::from_str::<GenerationPlan>(cleaned) {
        Ok(plan) => {
            if plan.mode != "single" && plan.mode != "multi" {
                return Err(format!("Invalid planner mode '{}'", plan.mode));
            }
            Ok(plan)
        }
        Err(first_err) => {
            // Tertiary attempt for truncated JSON fragments (common EOF parser failures).
            if let Some(repaired) = try_repair_json_fragment(cleaned) {
                if let Ok(plan) = serde_json::from_str::<GenerationPlan>(&repaired) {
                    if plan.mode != "single" && plan.mode != "multi" {
                        return Err(format!("Invalid planner mode '{}'", plan.mode));
                    }
                    return Ok(plan);
                }
            }

            // Secondary attempt: extract first outer JSON object if the model wrapped text around it.
            if let (Some(start), Some(end)) = (cleaned.find('{'), cleaned.rfind('}')) {
                if start < end {
                    let candidate = &cleaned[start..=end];
                    if let Ok(plan) = serde_json::from_str::<GenerationPlan>(candidate) {
                        if plan.mode != "single" && plan.mode != "multi" {
                            return Err(format!("Invalid planner mode '{}'", plan.mode));
                        }
                        return Ok(plan);
                    }
                }
            }
            Err(format!("Planner JSON parse failed: {}", first_err))
        }
    }
}

/// Extract a Python code block from an AI response.
fn extract_code_from_response(response: &str) -> Option<String> {
    crate::agent::extract::extract_code(response)
}

struct PartAcceptanceArtifact {
    code: String,
    stl_base64: Option<String>,
    post_geometry_report: Option<executor::PostGeometryValidationReport>,
    post_check_warning: Option<String>,
    semantic_findings: Vec<String>,
    retry_ladder_stage_reached: Option<u32>,
}

async fn evaluate_part_acceptance(
    part_code: &str,
    ctx: &executor::ExecutionContext,
    system_prompt: &str,
    part_request: &str,
    part_name: &str,
    semantic_contract: Option<&semantic_validate::SemanticPartContract>,
) -> Result<PartAcceptanceArtifact, String> {
    let no_event = |_evt: executor::ValidationEvent| {};
    let bbox_hint_owned = build_part_bbox_hint(
        semantic_contract,
        part_request,
        &ctx.config.semantic_bbox_mode,
    );
    let validation = executor::validate_and_retry(
        part_code.to_string(),
        ctx,
        system_prompt,
        bbox_hint_owned.as_deref(),
        &no_event,
    )
    .await
    .map_err(|e| format!("part acceptance validation error: {}", e))?;

    if !validation.success {
        return Err(validation
            .error
            .unwrap_or_else(|| "part validation failed".to_string()));
    }

    let mut semantic_findings = Vec::new();
    if ctx.config.quality_gates_strict && ctx.config.semantic_contract_strict {
        let report = validation.post_geometry_report.as_ref().ok_or_else(|| {
            "semantic validation unavailable: post-geometry report missing".to_string()
        })?;
        let contract = semantic_contract
            .cloned()
            .unwrap_or_else(|| semantic_validate::build_default_contract(part_name, part_request));
        let semantic =
            semantic_validate::validate_part_semantics(&contract, report, &validation.code);
        semantic_findings = semantic.findings.clone();
        if !semantic.passed {
            return Err(format!(
                "semantic validation failed: {}",
                semantic.findings.join("; ")
            ));
        }
    }

    Ok(PartAcceptanceArtifact {
        code: validation.code,
        stl_base64: validation.stl_base64,
        post_geometry_report: validation.post_geometry_report,
        post_check_warning: validation.post_check_warning,
        semantic_findings,
        retry_ladder_stage_reached: validation.retry_ladder_stage_reached,
    })
}

async fn build_part_preview_stl_with_repair(
    part_code: &str,
    ctx: &executor::ExecutionContext,
    system_prompt: &str,
    part_request: &str,
    part_name: &str,
    semantic_contract: Option<&semantic_validate::SemanticPartContract>,
) -> Result<String, String> {
    match evaluate_part_acceptance(
        part_code,
        ctx,
        system_prompt,
        part_request,
        part_name,
        semantic_contract,
    )
    .await
    {
        Ok(artifact) => artifact
            .stl_base64
            .ok_or_else(|| "validated part preview is missing STL output".to_string()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_assembly_bbox_hint, parse_plan, request_requires_multipart_contract,
        GenerationPlan, PartSpec,
    };

    #[test]
    fn parse_plan_accepts_valid_json() {
        let json = r#"{"mode":"multi","parts":[{"name":"body","description":"main","position":[0,0,0],"constraints":[]}],"description":"test"}"#;
        let plan = parse_plan(json).expect("plan should parse");
        assert_eq!(plan.mode, "multi");
        assert_eq!(plan.parts.len(), 1);
        assert_eq!(plan.parts[0].name, "body");
    }

    #[test]
    fn parse_plan_accepts_markdown_wrapped_json() {
        let json = r#"```json
{"mode":"single"}
```"#;
        let plan = parse_plan(json).expect("wrapped json should parse");
        assert_eq!(plan.mode, "single");
    }

    #[test]
    fn parse_plan_rejects_invalid_mode() {
        let json = r#"{"mode":"unknown","parts":[]}"#;
        assert!(parse_plan(json).is_err());
    }

    #[test]
    fn parse_plan_repairs_truncated_json() {
        let truncated = r#"{"mode":"multi","description":"x","parts":[{"name":"housing","description":"main","position":[0,0,0],"constraints":[]}"#;
        let parsed = parse_plan(truncated).expect("should repair truncated planner json");
        assert_eq!(parsed.mode, "multi");
        assert_eq!(parsed.parts.len(), 1);
        assert_eq!(parsed.parts[0].name, "housing");
    }

    #[test]
    fn multipart_contract_detects_explicit_separate_parts() {
        let user = "Make a wearable housing with a separate back plate";
        assert!(request_requires_multipart_contract(user, ""));
    }

    #[test]
    fn multipart_contract_not_required_for_simple_single_object() {
        let user = "Create a rounded enclosure with fillets";
        assert!(!request_requires_multipart_contract(user, ""));
    }

    // -----------------------------------------------------------------------
    // Whoop prompt integration tests
    // -----------------------------------------------------------------------

    const WHOOP_PROMPT: &str = r#"Create a fully parametric, editable CAD model of a wrist-worn fitness tracker housing with a snap-fit back plate, inspired by Whoop band design.
- Units: millimeters
- housing_length=42
- housing_width=28
- height_center=7.5
- height_ends=5
- wall=1.8
- top_thk=1.5
- corner_r=5
- back_plate_thk=1.5
- back_lip=1.5
- snap_tolerance=0.15
- oring_width=1.2
- oring_depth=0.8
- button_length=12
- button_width=4
- button_offset=6
- indicator_depth=0.3
- band_slot_width=20
- band_slot_height=2.5
- band_slot_depth=5
- Create two separate solids/bodies: Housing and BackPlate."#;

    #[test]
    fn whoop_prompt_triggers_multipart_contract() {
        assert!(
            request_requires_multipart_contract(WHOOP_PROMPT, ""),
            "Whoop prompt with 'separate solids' and 'back plate' must trigger multipart contract"
        );
    }

    #[test]
    fn whoop_prompt_triggers_multipart_contract_via_plan() {
        let plan_text = "housing_length=42\nhousing_width=28\nback plate ledge\nseparate bodies";
        assert!(request_requires_multipart_contract("", plan_text));
    }

    #[test]
    fn assembly_produces_valid_code() {
        use super::assemble_parts;
        let mock_parts: Vec<(String, String, [f64; 3])> = vec![
            (
                "housing".to_string(),
                "import cadquery as cq\nresult = cq.Workplane('XY').box(10, 10, 5)".to_string(),
                [0.0, 0.0, 0.0],
            ),
            (
                "back_plate".to_string(),
                "import cadquery as cq\nresult = cq.Workplane('XY').box(10, 10, 5)".to_string(),
                [0.0, 0.0, 0.0],
            ),
        ];

        let assembled = assemble_parts(&mock_parts).expect("assembly should succeed");
        assert!(assembled.contains("cq.Assembly()"));
        assert!(assembled.contains("part_housing"));
        assert!(assembled.contains("part_back_plate"));
        assert!(assembled.contains("assy.toCompound()"));
    }

    #[test]
    fn assembly_contract_validates_assembly() {
        use super::{assemble_parts, assembly_contract_issues};
        let mock_parts: Vec<(String, String, [f64; 3])> = vec![
            (
                "housing".to_string(),
                "import cadquery as cq\nresult = cq.Workplane('XY').box(10, 10, 5)".to_string(),
                [0.0, 0.0, 0.0],
            ),
            (
                "back_plate".to_string(),
                "import cadquery as cq\nresult = cq.Workplane('XY').box(10, 10, 5)".to_string(),
                [0.0, 0.0, 0.0],
            ),
        ];

        let assembled = assemble_parts(&mock_parts).unwrap();
        let issues = assembly_contract_issues(&assembled, &mock_parts);
        assert!(
            issues.is_empty(),
            "assembled code should pass contract validation, got: {:?}",
            issues
        );
    }

    // -----------------------------------------------------------------------
    // Edge case: no code extracted
    // -----------------------------------------------------------------------

    #[test]
    fn parse_plan_handles_empty_parts_gracefully() {
        let json = r#"{"mode":"multi","parts":[]}"#;
        let plan = parse_plan(json).expect("empty parts should parse");
        assert_eq!(plan.mode, "multi");
        assert!(plan.parts.is_empty());
    }

    #[test]
    fn parse_plan_extracts_json_from_prose() {
        let wrapped = r#"Here is the plan:
{"mode":"multi","description":"two parts","parts":[
  {"name":"housing","description":"main body","position":[0,0,0],"constraints":[]},
  {"name":"back_plate","description":"cover","position":[0,0,0],"constraints":[]}
]}
That should work."#;
        let plan = parse_plan(wrapped).expect("should extract JSON from surrounding prose");
        assert_eq!(plan.mode, "multi");
        assert_eq!(plan.parts.len(), 2);
    }

    #[test]
    fn parse_plan_repairs_truncated_multi_part() {
        let truncated = r#"{"mode":"multi","description":"housing + plate","parts":[{"name":"housing","description":"main","position":[0,0,0],"constraints":[]},{"name":"back_plate","description":"cover","position":[0,0,0"#;
        let result = parse_plan(truncated);
        match result {
            Ok(plan) => {
                assert_eq!(plan.mode, "multi");
                assert!(!plan.parts.is_empty());
            }
            Err(err) => {
                assert!(!err.trim().is_empty(), "parse error should be descriptive");
            }
        }
    }

    #[test]
    fn semantic_bbox_hint_prefers_envelope_dimensions() {
        let plan = GenerationPlan {
            mode: "multi".to_string(),
            description: Some("Two parts".to_string()),
            parts: vec![
                PartSpec {
                    name: "housing".to_string(),
                    description: "Primary shell with outer dimensions 42x28x7.5mm and wall thickness 1.8mm.".to_string(),
                    position: [0.0, 0.0, 0.0],
                    constraints: vec![],
                },
                PartSpec {
                    name: "cover".to_string(),
                    description: "Cover plate outer dimensions 30x24x1.5mm with lip height 1.2mm.".to_string(),
                    position: [0.0, 0.0, 0.0],
                    constraints: vec![],
                },
            ],
        };
        let hint = build_assembly_bbox_hint(
            &plan,
            "wall 1.8mm and lip 1.2mm",
            &crate::config::SemanticBboxMode::SemanticAware,
        )
        .expect("semantic bbox hint should be available");
        assert!(hint.contains("42"));
        assert!(hint.contains("28"));
        assert!(hint.contains("7.5"));
    }

    #[test]
    fn multipart_contract_detects_norwegian_keywords() {
        let user = "Lag en bakplate og eksplodert visning";
        assert!(
            request_requires_multipart_contract(user, ""),
            "Norwegian keywords 'bakplate' and 'eksplodert' should trigger multipart"
        );
    }

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
    let cq_version = state.cadquery_version.lock().unwrap().clone();
    let mut system_prompt = crate::agent::prompts::build_compact_system_prompt_for_preset(
        config.agent_rules_preset.as_deref(),
        cq_version.as_deref(),
    );
    let retrieval_query = format!("{}\n\n{}", user_request, design_plan_text);
    let retrieval_result = retrieval::retrieve_context(
        &retrieval_query,
        &config,
        config.agent_rules_preset.as_deref(),
        cq_version.as_deref(),
    )
    .await;
    if !retrieval_result.context_markdown.is_empty() {
        system_prompt.push_str("\n\n");
        system_prompt.push_str(&retrieval_result.context_markdown);
    }

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

    let on_iter_event = |evt: iterative::IterativeEvent| match evt {
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

// ---------------------------------------------------------------------------
// Retry a single failed part
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn retry_part(
    part_index: usize,
    part_spec: PartSpec,
    design_plan_text: String,
    _user_request: String,
    on_event: Channel<MultiPartEvent>,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let config = state.config.lock().unwrap().clone();
    let cq_version = state.cadquery_version.lock().unwrap().clone();
    let mut system_prompt = prompts::build_compact_system_prompt_for_preset(
        config.agent_rules_preset.as_deref(),
        cq_version.as_deref(),
    );
    let provider_id = config.ai_provider.clone();
    let model_id = config.model.clone();
    let mut total_usage = TokenUsage::default();

    let retrieval_query = format!("{}\n\n{}", design_plan_text, part_spec.description);
    let retrieval_result = retrieval::retrieve_context(
        &retrieval_query,
        &config,
        config.agent_rules_preset.as_deref(),
        cq_version.as_deref(),
    )
    .await;
    if !retrieval_result.context_markdown.is_empty() {
        system_prompt.push_str("\n\n");
        system_prompt.push_str(&retrieval_result.context_markdown);
    }

    // Build part prompt
    let part_prompt = build_part_prompt(&system_prompt, &part_spec, &design_plan_text, &config);

    let part_messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: part_prompt,
        },
        ChatMessage {
            role: "user".to_string(),
            content: format!("Generate the CadQuery code for: {}", part_spec.description),
        },
    ];

    // Stream generation for the single part
    let provider = create_provider(&config)?;
    let (tx, mut rx) = mpsc::channel::<StreamDelta>(100);
    let provider_handle = tokio::spawn(async move { provider.stream(&part_messages, tx).await });

    let mut full_response = String::new();
    while let Some(delta) = rx.recv().await {
        full_response.push_str(&delta.content);
        let _ = on_event.send(MultiPartEvent::PartDelta {
            part_index,
            part_name: part_spec.name.clone(),
            delta: delta.content,
        });
    }

    match provider_handle.await {
        Ok(Ok(stream_usage)) => {
            if let Some(ref u) = stream_usage {
                total_usage.add(u);
                emit_usage(&on_event, "retry_part", u, &provider_id, &model_id);
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

    // Extract code
    let mut code = extract_code_from_response(&full_response);
    if code.is_none() {
        let _ = on_event.send(MultiPartEvent::PlanStatus {
            message: format!(
                "Part '{}' returned non-code output. Requesting strict code-only retry...",
                part_spec.name
            ),
        });
        let (retried, usage) = request_code_only_part_retry(
            &config,
            &system_prompt,
            &part_spec,
            &design_plan_text,
            &full_response,
        )
        .await?;
        if let Some(ref u) = usage {
            total_usage.add(u);
            emit_usage(
                &on_event,
                "retry_part_code_recovery",
                u,
                &provider_id,
                &model_id,
            );
        }
        code = retried;
    }
    match code {
        Some(c) => {
            let _ = on_event.send(MultiPartEvent::PartCodeExtracted {
                part_index,
                part_name: part_spec.name.clone(),
                code: c.clone(),
            });
            let _ = on_event.send(MultiPartEvent::PartComplete {
                part_index,
                part_name: part_spec.name.clone(),
                success: true,
                error: None,
            });

            // Run STL execution for retried part and await completion so preview event is delivered.
            let venv_path = state.venv_path.lock().unwrap().clone();
            if let Some(venv_dir) = venv_path {
                if let Ok(runner_script) = super::find_python_script("runner.py") {
                    let part_code = c.clone();
                    let part_name = part_spec.name.clone();
                    let evt_channel = on_event.clone();
                    let pi = part_index;
                    let preview_ctx = executor::ExecutionContext {
                        venv_dir,
                        runner_script,
                        config: config.clone(),
                    };
                    let semantic_contract = semantic_validate::build_default_contract(
                        &part_name,
                        &part_spec.description,
                    );
                    match build_part_preview_stl_with_repair(
                        &part_code,
                        &preview_ctx,
                        &system_prompt,
                        &part_spec.description,
                        &part_name,
                        Some(&semantic_contract),
                    )
                    .await
                    {
                        Ok(stl_base64) => {
                            let _ = evt_channel.send(MultiPartEvent::PartStlReady {
                                part_index: pi,
                                part_name,
                                stl_base64,
                            });
                        }
                        Err(e) => {
                            let _ = evt_channel.send(MultiPartEvent::PartStlFailed {
                                part_index: pi,
                                part_name: part_name.clone(),
                                error: e,
                            });
                        }
                    }
                }
            }

            if total_usage.total() > 0 {
                emit_usage(&on_event, "total", &total_usage, &provider_id, &model_id);
            }

            let _ = on_event.send(MultiPartEvent::Done {
                success: true,
                error: None,
                validated: false,
            });

            Ok(c)
        }
        None => {
            let _ = on_event.send(MultiPartEvent::PartComplete {
                part_index,
                part_name: part_spec.name.clone(),
                success: false,
                error: Some("No code block found in response".to_string()),
            });
            let _ = on_event.send(MultiPartEvent::Done {
                success: false,
                error: Some("No code extracted from retry response".to_string()),
                validated: false,
            });
            Err(AppError::AiProviderError(
                "No code extracted from retry response".to_string(),
            ))
        }
    }
}
