use base64::Engine;
use serde::Serialize;

use crate::agent::executor::{self, ExecutionContext};
use crate::agent::extract;
use crate::ai::message::ChatMessage;
use crate::ai::provider::TokenUsage;
use crate::commands::chat::create_provider_with_temp;
use crate::config::AppConfig;
use crate::error::AppError;

const CONSERVATIVE_TEMP: f32 = 0.3;
const CREATIVE_TEMP: f32 = 0.8;

/// CadQuery operations used for scoring code complexity.
const CADQUERY_OPS: &[&str] = &[
    ".box(",
    ".cylinder(",
    ".sphere(",
    ".fillet(",
    ".chamfer(",
    ".shell(",
    ".cut(",
    ".union(",
    ".intersect(",
    ".loft(",
    ".sweep(",
    ".revolve(",
    ".extrude(",
    ".hole(",
    ".rect(",
    ".circle(",
    ".translate(",
    ".rotate(",
    ".mirror(",
];

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ScoreBreakdown {
    pub op_count: u32,
    pub line_count: u32,
}

impl ScoreBreakdown {
    pub fn total(&self) -> u32 {
        self.op_count * 10 + self.line_count
    }
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub label: String,
    pub temperature: f32,
    pub response: Option<String>,
    pub code: Option<String>,
    pub execution_success: bool,
    pub stl_base64: Option<String>,
    pub score: ScoreBreakdown,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone)]
pub struct ConsensusResult {
    pub winner: Candidate,
    pub loser: Candidate,
    pub total_usage: TokenUsage,
}

/// Events emitted during consensus to update the frontend.
#[derive(Debug, Clone)]
pub enum ConsensusEvent {
    Started {
        candidate_count: u32,
    },
    CandidateUpdate {
        label: String,
        temperature: f32,
        status: String,
        has_code: Option<bool>,
        execution_success: Option<bool>,
    },
    Winner {
        label: String,
        score: u32,
        reason: String,
    },
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

pub fn score_code(code: &str) -> ScoreBreakdown {
    let lower = code.to_lowercase();
    let op_count = CADQUERY_OPS
        .iter()
        .filter(|op| lower.contains(&op.to_lowercase()))
        .count() as u32;

    let line_count = code
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        })
        .count() as u32;

    ScoreBreakdown {
        op_count,
        line_count,
    }
}

/// Select the winner between two candidates.
/// Returns (winner_label, reason).
pub fn select_winner<'a>(
    a: &'a Candidate,
    b: &'a Candidate,
) -> (&'a Candidate, &'a Candidate, String) {
    const EXECUTION_BONUS: u32 = 1000;

    let score_a = a.score.total()
        + if a.execution_success {
            EXECUTION_BONUS
        } else {
            0
        };
    let score_b = b.score.total()
        + if b.execution_success {
            EXECUTION_BONUS
        } else {
            0
        };

    if score_a >= score_b {
        let reason = if a.execution_success && !b.execution_success {
            format!("Candidate {} executed successfully", a.label)
        } else if a.code.is_some() && b.code.is_none() {
            format!("Candidate {} produced code", a.label)
        } else {
            format!(
                "Candidate {} scored higher ({} vs {})",
                a.label, score_a, score_b
            )
        };
        (a, b, reason)
    } else {
        let reason = if b.execution_success && !a.execution_success {
            format!("Candidate {} executed successfully", b.label)
        } else if b.code.is_some() && a.code.is_none() {
            format!("Candidate {} produced code", b.label)
        } else {
            format!(
                "Candidate {} scored higher ({} vs {})",
                b.label, score_b, score_a
            )
        };
        (b, a, reason)
    }
}

// ---------------------------------------------------------------------------
// Core
// ---------------------------------------------------------------------------

pub async fn run_consensus(
    messages: &[ChatMessage],
    config: &AppConfig,
    ctx: &ExecutionContext,
    on_event: &(dyn Fn(ConsensusEvent) + Send + Sync),
) -> Result<ConsensusResult, AppError> {
    on_event(ConsensusEvent::Started { candidate_count: 2 });

    // Create two providers at different temperatures
    let provider_a = create_provider_with_temp(config, Some(CONSERVATIVE_TEMP))?;
    let provider_b = create_provider_with_temp(config, Some(CREATIVE_TEMP))?;

    let messages_a = messages.to_vec();
    let messages_b = messages.to_vec();

    // Emit generating status for both candidates
    on_event(ConsensusEvent::CandidateUpdate {
        label: "A".to_string(),
        temperature: CONSERVATIVE_TEMP,
        status: "generating".to_string(),
        has_code: None,
        execution_success: None,
    });
    on_event(ConsensusEvent::CandidateUpdate {
        label: "B".to_string(),
        temperature: CREATIVE_TEMP,
        status: "generating".to_string(),
        has_code: None,
        execution_success: None,
    });

    // Run both completions in parallel
    let handle_a = tokio::spawn(async move { provider_a.complete(&messages_a, None).await });
    let handle_b = tokio::spawn(async move { provider_b.complete(&messages_b, None).await });

    let (result_a, result_b) = tokio::join!(handle_a, handle_b);

    // Process candidate A
    let (response_a, usage_a) = match result_a {
        Ok(Ok((text, usage))) => (Some(text), usage.unwrap_or_default()),
        Ok(Err(e)) => {
            eprintln!("[consensus] Candidate A failed: {}", e);
            (None, TokenUsage::default())
        }
        Err(e) => {
            eprintln!("[consensus] Candidate A panicked: {}", e);
            (None, TokenUsage::default())
        }
    };

    // Process candidate B
    let (response_b, usage_b) = match result_b {
        Ok(Ok((text, usage))) => (Some(text), usage.unwrap_or_default()),
        Ok(Err(e)) => {
            eprintln!("[consensus] Candidate B failed: {}", e);
            (None, TokenUsage::default())
        }
        Err(e) => {
            eprintln!("[consensus] Candidate B panicked: {}", e);
            (None, TokenUsage::default())
        }
    };

    // Extract code from each response
    let code_a = response_a.as_ref().and_then(|r| extract::extract_code(r));
    let code_b = response_b.as_ref().and_then(|r| extract::extract_code(r));

    on_event(ConsensusEvent::CandidateUpdate {
        label: "A".to_string(),
        temperature: CONSERVATIVE_TEMP,
        status: "generated".to_string(),
        has_code: Some(code_a.is_some()),
        execution_success: None,
    });
    on_event(ConsensusEvent::CandidateUpdate {
        label: "B".to_string(),
        temperature: CREATIVE_TEMP,
        status: "generated".to_string(),
        has_code: Some(code_b.is_some()),
        execution_success: None,
    });

    // Execute each candidate sequentially (shared temp files)
    let mut candidate_a = Candidate {
        label: "A".to_string(),
        temperature: CONSERVATIVE_TEMP,
        response: response_a,
        code: code_a.clone(),
        execution_success: false,
        stl_base64: None,
        score: code_a
            .as_ref()
            .map(|c| score_code(c))
            .unwrap_or(ScoreBreakdown {
                op_count: 0,
                line_count: 0,
            }),
        usage: usage_a,
    };

    let mut candidate_b = Candidate {
        label: "B".to_string(),
        temperature: CREATIVE_TEMP,
        response: response_b,
        code: code_b.clone(),
        execution_success: false,
        stl_base64: None,
        score: code_b
            .as_ref()
            .map(|c| score_code(c))
            .unwrap_or(ScoreBreakdown {
                op_count: 0,
                line_count: 0,
            }),
        usage: usage_b,
    };

    // Execute candidate A
    if let Some(ref code) = candidate_a.code {
        on_event(ConsensusEvent::CandidateUpdate {
            label: "A".to_string(),
            temperature: CONSERVATIVE_TEMP,
            status: "executing".to_string(),
            has_code: Some(true),
            execution_success: None,
        });

        match executor::execute_with_timeout(code, &ctx.venv_dir, &ctx.runner_script).await {
            Ok(exec_result) => {
                if !exec_result.stl_data.is_empty() {
                    candidate_a.execution_success = true;
                    candidate_a.stl_base64 = Some(
                        base64::engine::general_purpose::STANDARD.encode(&exec_result.stl_data),
                    );
                }
            }
            Err(e) => {
                eprintln!("[consensus] Candidate A execution failed: {}", e);
            }
        }

        on_event(ConsensusEvent::CandidateUpdate {
            label: "A".to_string(),
            temperature: CONSERVATIVE_TEMP,
            status: "executed".to_string(),
            has_code: Some(true),
            execution_success: Some(candidate_a.execution_success),
        });
    }

    // Execute candidate B
    if let Some(ref code) = candidate_b.code {
        on_event(ConsensusEvent::CandidateUpdate {
            label: "B".to_string(),
            temperature: CREATIVE_TEMP,
            status: "executing".to_string(),
            has_code: Some(true),
            execution_success: None,
        });

        match executor::execute_with_timeout(code, &ctx.venv_dir, &ctx.runner_script).await {
            Ok(exec_result) => {
                if !exec_result.stl_data.is_empty() {
                    candidate_b.execution_success = true;
                    candidate_b.stl_base64 = Some(
                        base64::engine::general_purpose::STANDARD.encode(&exec_result.stl_data),
                    );
                }
            }
            Err(e) => {
                eprintln!("[consensus] Candidate B execution failed: {}", e);
            }
        }

        on_event(ConsensusEvent::CandidateUpdate {
            label: "B".to_string(),
            temperature: CREATIVE_TEMP,
            status: "executed".to_string(),
            has_code: Some(true),
            execution_success: Some(candidate_b.execution_success),
        });
    }

    // Select winner
    let (winner, loser, reason) = select_winner(&candidate_a, &candidate_b);

    on_event(ConsensusEvent::Winner {
        label: winner.label.clone(),
        score: winner.score.total(),
        reason: reason.clone(),
    });

    let mut total_usage = TokenUsage::default();
    total_usage.add(&candidate_a.usage);
    total_usage.add(&candidate_b.usage);

    Ok(ConsensusResult {
        winner: winner.clone(),
        loser: loser.clone(),
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
    fn test_score_simple_box() {
        let code = "import cadquery as cq\nresult = cq.Workplane('XY').box(10, 10, 10)";
        let score = score_code(code);
        assert_eq!(score.op_count, 1); // .box(
        assert_eq!(score.line_count, 2);
    }

    #[test]
    fn test_score_complex_code() {
        let code = r#"import cadquery as cq
result = (
    cq.Workplane("XY")
    .box(50, 30, 20)
    .fillet(2)
    .cut(cq.Workplane("XY").cylinder(10, 100))
    .translate((0, 0, 5))
)
"#;
        let score = score_code(code);
        assert!(score.op_count >= 4); // box, fillet, cut, cylinder, translate
        assert!(score.line_count >= 6);
    }

    #[test]
    fn test_score_empty_code() {
        let score = score_code("");
        assert_eq!(score.op_count, 0);
        assert_eq!(score.line_count, 0);
    }

    #[test]
    fn test_score_comments_excluded() {
        let code = "# This is a comment\n# Another comment\nimport cadquery as cq\nresult = cq.Workplane('XY').box(10, 10, 10)";
        let score = score_code(code);
        assert_eq!(score.line_count, 2); // comments excluded
    }

    #[test]
    fn test_select_winner_execution_beats_score() {
        let a = Candidate {
            label: "A".to_string(),
            temperature: 0.3,
            response: Some("code A".to_string()),
            code: Some(".box(".to_string()),
            execution_success: true,
            stl_base64: None,
            score: ScoreBreakdown {
                op_count: 1,
                line_count: 1,
            },
            usage: TokenUsage::default(),
        };
        let b = Candidate {
            label: "B".to_string(),
            temperature: 0.8,
            response: Some("code B".to_string()),
            code: Some(".box(.fillet(.cut(.cylinder(.translate(".to_string()),
            execution_success: false,
            stl_base64: None,
            score: ScoreBreakdown {
                op_count: 5,
                line_count: 10,
            },
            usage: TokenUsage::default(),
        };
        let (winner, _loser, reason) = select_winner(&a, &b);
        assert_eq!(winner.label, "A");
        assert!(reason.contains("executed successfully"));
    }

    #[test]
    fn test_select_winner_higher_score_wins() {
        let a = Candidate {
            label: "A".to_string(),
            temperature: 0.3,
            response: Some("code A".to_string()),
            code: Some(".box(".to_string()),
            execution_success: false,
            stl_base64: None,
            score: ScoreBreakdown {
                op_count: 1,
                line_count: 1,
            },
            usage: TokenUsage::default(),
        };
        let b = Candidate {
            label: "B".to_string(),
            temperature: 0.8,
            response: Some("code B".to_string()),
            code: Some("complex code".to_string()),
            execution_success: false,
            stl_base64: None,
            score: ScoreBreakdown {
                op_count: 5,
                line_count: 10,
            },
            usage: TokenUsage::default(),
        };
        let (winner, _loser, reason) = select_winner(&a, &b);
        assert_eq!(winner.label, "B");
        assert!(reason.contains("scored higher"));
    }

    #[test]
    fn test_select_winner_no_code() {
        let a = Candidate {
            label: "A".to_string(),
            temperature: 0.3,
            response: Some("code A".to_string()),
            code: Some(".box(".to_string()),
            execution_success: false,
            stl_base64: None,
            score: ScoreBreakdown {
                op_count: 1,
                line_count: 1,
            },
            usage: TokenUsage::default(),
        };
        let b = Candidate {
            label: "B".to_string(),
            temperature: 0.8,
            response: None,
            code: None,
            execution_success: false,
            stl_base64: None,
            score: ScoreBreakdown {
                op_count: 0,
                line_count: 0,
            },
            usage: TokenUsage::default(),
        };
        let (winner, _loser, reason) = select_winner(&a, &b);
        assert_eq!(winner.label, "A");
        assert!(reason.contains("scored higher") || reason.contains("produced code"));
    }
}
