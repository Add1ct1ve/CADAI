use crate::agent::validate::ErrorCategory;
use regex::Regex;

/// A single generation attempt within the session.
#[derive(Debug, Clone)]
pub struct GenerationAttempt {
    pub user_request: String,
    pub operations_used: Vec<String>,
    pub success: bool,
    pub error_category: Option<ErrorCategory>,
    pub failing_operation: Option<String>,
    pub error_summary: Option<String>,
}

/// In-memory session memory — tracks generation outcomes within a conversation.
pub struct SessionMemory {
    attempts: Vec<GenerationAttempt>,
}

impl SessionMemory {
    pub fn new() -> Self {
        Self {
            attempts: Vec::new(),
        }
    }

    /// Record a generation attempt. Caps at 20 entries (drops oldest).
    pub fn record_attempt(&mut self, attempt: GenerationAttempt) {
        self.attempts.push(attempt);
        if self.attempts.len() > 20 {
            self.attempts.remove(0);
        }
    }

    /// Build a context section for injection into the system prompt.
    /// Returns `None` if no attempts have been recorded.
    pub fn build_context_section(&self) -> Option<String> {
        if self.attempts.is_empty() {
            return None;
        }

        let mut out = String::new();
        out.push_str("## Session Context\nPrevious generation attempts in this conversation:\n");

        for (i, attempt) in self.attempts.iter().enumerate() {
            let ops = if attempt.operations_used.is_empty() {
                "no ops detected".to_string()
            } else {
                attempt.operations_used.join(", ")
            };
            let status = if attempt.success {
                "SUCCESS".to_string()
            } else {
                match (&attempt.failing_operation, &attempt.error_summary) {
                    (Some(op), Some(summary)) => {
                        format!("FAILED ({} failure: {})", op, summary)
                    }
                    (Some(op), None) => format!("FAILED ({} failure)", op),
                    (None, Some(summary)) => format!("FAILED ({})", summary),
                    (None, None) => "FAILED".to_string(),
                }
            };
            out.push_str(&format!(
                "{}. \"{}\" — {} → {}\n",
                i + 1,
                attempt.user_request,
                ops,
                status,
            ));
        }

        // Build learnings
        let learnings = self.build_learnings();
        if !learnings.is_empty() {
            out.push_str("\nSession learnings:\n");
            for learning in &learnings {
                out.push_str(&format!("- {}\n", learning));
            }
        }

        out.push_str("\nApply these learnings. Do NOT repeat failed approaches.");

        Some(out)
    }

    /// Get unique list of operations that caused failures.
    pub fn failed_operations(&self) -> Vec<String> {
        let mut ops: Vec<String> = self
            .attempts
            .iter()
            .filter(|a| !a.success)
            .filter_map(|a| a.failing_operation.clone())
            .collect();
        ops.sort();
        ops.dedup();
        ops
    }

    /// Clear all recorded attempts.
    pub fn reset(&mut self) {
        self.attempts.clear();
    }

    /// Build learning bullet points from attempts (capped at 5).
    fn build_learnings(&self) -> Vec<String> {
        let mut learnings = Vec::new();

        // Collect failed operations with their error summaries
        let mut seen_failures: Vec<String> = Vec::new();
        for attempt in &self.attempts {
            if !attempt.success {
                if let Some(ref op) = attempt.failing_operation {
                    let key = op.clone();
                    if !seen_failures.contains(&key) {
                        seen_failures.push(key);
                        let summary = attempt.error_summary.as_deref().unwrap_or("unknown reason");
                        learnings.push(format!("{}() failed — {}", op, summary));
                    }
                }
            }
        }

        // Collect reliable operation combos from successes
        let mut reliable_combos: Vec<String> = Vec::new();
        for attempt in &self.attempts {
            if attempt.success && !attempt.operations_used.is_empty() {
                let combo = attempt.operations_used.join(" + ");
                if !reliable_combos.contains(&combo) {
                    reliable_combos.push(combo);
                }
            }
        }
        for combo in &reliable_combos {
            if learnings.len() < 5 {
                learnings.push(format!("{} is a reliable combination", combo));
            }
        }

        learnings.truncate(5);
        learnings
    }
}

/// Extract CAD operation names from Python code.
/// Matches both standalone function calls (Build123d) and method-chain patterns.
pub fn extract_operations_from_code(code: &str) -> Vec<String> {
    // Match both `.operation(` (method chain) and standalone `operation(` (Build123d)
    let pattern = Regex::new(r"(?:^|[^.\w])(\w+)\s*\(").unwrap();
    let known_ops = [
        "extrude", "revolve", "loft", "sweep", "shell", "fillet", "chamfer", "cut", "union",
        "hole", "tag",
    ];

    let mut ops = Vec::new();
    for cap in pattern.captures_iter(code) {
        let op = &cap[1];
        if known_ops.contains(&op) && !ops.contains(&op.to_string()) {
            ops.push(op.to_string());
        }
    }
    ops
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::validate::{ErrorCategory, TopologySubKind};

    #[test]
    fn test_empty_memory_returns_none() {
        let mem = SessionMemory::new();
        assert!(mem.build_context_section().is_none());
    }

    #[test]
    fn test_record_and_build_context() {
        let mut mem = SessionMemory::new();
        mem.record_attempt(GenerationAttempt {
            user_request: "Make a vase".to_string(),
            operations_used: vec!["revolve".to_string(), "fillet".to_string()],
            success: true,
            error_category: None,
            failing_operation: None,
            error_summary: None,
        });
        mem.record_attempt(GenerationAttempt {
            user_request: "Make a hollow box".to_string(),
            operations_used: vec!["loft".to_string(), "shell".to_string()],
            success: false,
            error_category: Some(ErrorCategory::Topology(TopologySubKind::ShellFailure)),
            failing_operation: Some("shell".to_string()),
            error_summary: Some("shell on lofted body".to_string()),
        });

        let section = mem.build_context_section().unwrap();
        assert!(section.contains("Make a vase"));
        assert!(section.contains("SUCCESS"));
        assert!(section.contains("Make a hollow box"));
        assert!(section.contains("FAILED"));
        assert!(section.contains("shell"));
    }

    #[test]
    fn test_max_20_attempts() {
        let mut mem = SessionMemory::new();
        for i in 0..25 {
            mem.record_attempt(GenerationAttempt {
                user_request: format!("Request {}", i),
                operations_used: vec![],
                success: true,
                error_category: None,
                failing_operation: None,
                error_summary: None,
            });
        }
        assert_eq!(mem.attempts.len(), 20);
        // Oldest (0-4) should be dropped
        assert!(mem.attempts[0].user_request.contains("5"));
    }

    #[test]
    fn test_reset_clears_all() {
        let mut mem = SessionMemory::new();
        mem.record_attempt(GenerationAttempt {
            user_request: "test".to_string(),
            operations_used: vec![],
            success: true,
            error_category: None,
            failing_operation: None,
            error_summary: None,
        });
        assert!(mem.build_context_section().is_some());
        mem.reset();
        assert!(mem.build_context_section().is_none());
    }

    #[test]
    fn test_extract_operations_from_code() {
        let code = r#"
from build123d import *
with BuildPart() as p:
    with BuildSketch():
        Circle(20)
    revolve(axis=Axis.X)
    fillet(p.edges(), radius=2)
    shell(p.faces(), thickness=-1)
result = p.part
"#;
        let ops = extract_operations_from_code(code);
        assert!(ops.contains(&"revolve".to_string()));
        assert!(ops.contains(&"fillet".to_string()));
        assert!(ops.contains(&"shell".to_string()));
    }

    #[test]
    fn test_extract_operations_empty_code() {
        let ops = extract_operations_from_code("");
        assert!(ops.is_empty());
    }

    #[test]
    fn test_failed_operations_list() {
        let mut mem = SessionMemory::new();
        mem.record_attempt(GenerationAttempt {
            user_request: "test1".to_string(),
            operations_used: vec!["shell".to_string()],
            success: false,
            error_category: None,
            failing_operation: Some("shell".to_string()),
            error_summary: None,
        });
        mem.record_attempt(GenerationAttempt {
            user_request: "test2".to_string(),
            operations_used: vec!["loft".to_string()],
            success: false,
            error_category: None,
            failing_operation: Some("loft".to_string()),
            error_summary: None,
        });
        mem.record_attempt(GenerationAttempt {
            user_request: "test3".to_string(),
            operations_used: vec!["shell".to_string()],
            success: false,
            error_category: None,
            failing_operation: Some("shell".to_string()),
            error_summary: None,
        });

        let failed = mem.failed_operations();
        assert_eq!(failed.len(), 2);
        assert!(failed.contains(&"shell".to_string()));
        assert!(failed.contains(&"loft".to_string()));
    }

    #[test]
    fn test_context_section_includes_learnings() {
        let mut mem = SessionMemory::new();
        mem.record_attempt(GenerationAttempt {
            user_request: "Make a box".to_string(),
            operations_used: vec!["extrude".to_string(), "fillet".to_string()],
            success: true,
            error_category: None,
            failing_operation: None,
            error_summary: None,
        });
        mem.record_attempt(GenerationAttempt {
            user_request: "Make a hollow box".to_string(),
            operations_used: vec!["extrude".to_string(), "shell".to_string()],
            success: false,
            error_category: Some(ErrorCategory::Topology(TopologySubKind::ShellFailure)),
            failing_operation: Some("shell".to_string()),
            error_summary: Some("shell on complex body".to_string()),
        });

        let section = mem.build_context_section().unwrap();
        assert!(section.contains("Session learnings:"));
        assert!(section.contains("shell() failed"));
        assert!(section.contains("reliable combination"));
    }
}
