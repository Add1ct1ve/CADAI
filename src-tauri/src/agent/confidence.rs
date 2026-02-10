use serde::Serialize;

use crate::agent::design::{self, PlanValidation};
use crate::agent::rules::CookbookEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceLevel {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize)]
pub struct CookbookMatch {
    pub title: String,
    pub overlap_score: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfidenceAssessment {
    pub level: ConfidenceLevel,
    pub score: u32,
    pub cookbook_matches: Vec<CookbookMatch>,
    pub warnings: Vec<String>,
    pub message: String,
}

/// Assess generation confidence from plan validation and cookbook familiarity.
pub fn assess_confidence(
    validation: &PlanValidation,
    cookbook: Option<&[CookbookEntry]>,
) -> ConfidenceAssessment {
    // Base score from risk (risk 0 -> 100, risk 10 -> 0)
    let base_score = 100_i32 - (validation.risk_score as i32 * 10);

    // Cookbook matching
    let cookbook_matches = match cookbook {
        Some(entries) => match_cookbook(
            &validation.extracted_operations,
            "", // plan text not available here; operations suffice
            entries,
        ),
        None => vec![],
    };

    let distinct_ops = validation.extracted_operations.len();

    let cookbook_bonus: i32 = if cookbook_matches.len() >= 2 {
        15
    } else if cookbook_matches.len() == 1 {
        10
    } else if distinct_ops >= 3 {
        -10 // novel combination penalty
    } else {
        0
    };

    let final_score = (base_score + cookbook_bonus).clamp(0, 100) as u32;

    let level = if final_score >= 70 {
        ConfidenceLevel::High
    } else if final_score >= 40 {
        ConfidenceLevel::Medium
    } else {
        ConfidenceLevel::Low
    };

    // Build warnings for yellow/red
    let mut warnings = Vec::new();

    let has_loft = validation
        .extracted_operations
        .iter()
        .any(|op| op == "loft");
    let has_shell = validation
        .extracted_operations
        .iter()
        .any(|op| op == "shell");

    if has_loft && has_shell {
        warnings.push("This design uses loft + shell — may need retries".to_string());
    }

    if cookbook_matches.is_empty() && distinct_ops >= 3 {
        warnings.push("Novel operation combination — no matching cookbook pattern".to_string());
    }

    if distinct_ops >= 5 {
        warnings.push(format!(
            "High complexity: {} distinct operations",
            distinct_ops
        ));
    }

    let message = match level {
        ConfidenceLevel::High => "Simple geometry matching known patterns".to_string(),
        ConfidenceLevel::Medium => {
            if has_loft {
                "Moderate complexity — loft operation detected".to_string()
            } else if has_shell {
                "Moderate complexity — shell operation detected".to_string()
            } else {
                "Moderate complexity".to_string()
            }
        }
        ConfidenceLevel::Low => {
            "Complex design with risky operation combinations".to_string()
        }
    };

    ConfidenceAssessment {
        level,
        score: final_score,
        cookbook_matches,
        warnings,
        message,
    }
}

/// Match plan operations against cookbook recipes.
///
/// For each cookbook entry, extract operations from title + description,
/// compute overlap with the plan's operations, and return top matches.
pub fn match_cookbook(
    plan_operations: &[String],
    plan_text: &str,
    cookbook: &[CookbookEntry],
) -> Vec<CookbookMatch> {
    if plan_operations.is_empty() {
        return vec![];
    }

    let plan_ops_set: std::collections::HashSet<&str> =
        plan_operations.iter().map(|s| s.as_str()).collect();

    let plan_text_lower = plan_text.to_lowercase();

    let mut matches: Vec<CookbookMatch> = Vec::new();

    for entry in cookbook {
        // Extract operations from title + description
        let mut entry_text = entry.title.clone();
        if let Some(ref desc) = entry.description {
            entry_text.push(' ');
            entry_text.push_str(desc);
        }
        let entry_ops = design::extract_operations_from_text(&entry_text);
        let entry_ops_set: std::collections::HashSet<&str> =
            entry_ops.iter().map(|s| s.as_str()).collect();

        if entry_ops_set.is_empty() {
            continue;
        }

        // Overlap = |intersection| / |plan_ops|
        let intersection = plan_ops_set.intersection(&entry_ops_set).count();
        let mut overlap = intersection as f32 / plan_ops_set.len() as f32;

        // Boost if title words appear in plan text
        if !plan_text_lower.is_empty() {
            let title_lower = entry.title.to_lowercase();
            let title_words: Vec<&str> = title_lower
                .split_whitespace()
                .filter(|w| w.len() > 2)
                .collect();

            if !title_words.is_empty() {
                let matching_words = title_words
                    .iter()
                    .filter(|w| plan_text_lower.contains(*w))
                    .count();
                if matching_words * 2 >= title_words.len() {
                    // >= 50% title words match
                    overlap += 0.2;
                }
            }
        }

        if overlap >= 0.3 {
            matches.push(CookbookMatch {
                title: entry.title.clone(),
                overlap_score: overlap,
            });
        }
    }

    // Sort descending by overlap_score
    matches.sort_by(|a, b| {
        b.overlap_score
            .partial_cmp(&a.overlap_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Cap at top 3
    matches.truncate(3);

    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_validation(
        risk_score: u32,
        operations: Vec<&str>,
    ) -> PlanValidation {
        PlanValidation {
            is_valid: risk_score <= 7,
            risk_score,
            warnings: vec![],
            rejected_reason: None,
            extracted_operations: operations.into_iter().map(|s| s.to_string()).collect(),
            extracted_dimensions: vec![],
        }
    }

    fn make_cookbook() -> Vec<CookbookEntry> {
        vec![
            CookbookEntry {
                title: "Hollow box (shell operation)".to_string(),
                description: Some("Create a box and shell it to make hollow".to_string()),
                code: "import cadquery as cq\nresult = cq.Workplane('XY').box(10,10,10).shell(1)".to_string(),
            },
            CookbookEntry {
                title: "Revolve wine glass".to_string(),
                description: Some("Revolve a profile to create a wine glass shape".to_string()),
                code: "import cadquery as cq\nresult = cq.Workplane('XY').revolve()".to_string(),
            },
            CookbookEntry {
                title: "Loft between profiles".to_string(),
                description: Some("Loft between two different cross-section profiles".to_string()),
                code: "import cadquery as cq\nresult = cq.Workplane('XY').loft()".to_string(),
            },
            CookbookEntry {
                title: "Simple extrude box".to_string(),
                description: Some("Extrude a rectangle to create a box".to_string()),
                code: "import cadquery as cq\nresult = cq.Workplane('XY').box(10,10,10)".to_string(),
            },
            CookbookEntry {
                title: "Sweep pipe along path".to_string(),
                description: Some("Sweep a circular profile along a path to create a pipe".to_string()),
                code: "import cadquery as cq\nresult = cq.Workplane('XY').sweep()".to_string(),
            },
        ]
    }

    #[test]
    fn test_high_confidence_simple_box() {
        let validation = make_validation(0, vec!["extrude"]);
        let cookbook = make_cookbook();
        let result = assess_confidence(&validation, Some(&cookbook));
        assert!(result.score >= 70, "score {} should be >= 70", result.score);
        assert_eq!(result.level, ConfidenceLevel::High);
    }

    #[test]
    fn test_medium_confidence_revolve() {
        // revolve adds risk +1 → risk 1 → base 90, but revolve matches cookbook (+10) → 100
        // Actually need a risk that brings us to medium range
        let validation = make_validation(4, vec!["revolve"]);
        let cookbook = make_cookbook();
        let result = assess_confidence(&validation, Some(&cookbook));
        // base = 60, cookbook match +10 = 70 → actually High
        // Use higher risk to get medium
        let validation2 = make_validation(5, vec!["revolve"]);
        let result2 = assess_confidence(&validation2, Some(&cookbook));
        // base = 50, cookbook +10 = 60 → Medium
        assert!(
            result2.score >= 40 && result2.score < 70,
            "score {} should be 40-69",
            result2.score
        );
        assert_eq!(result2.level, ConfidenceLevel::Medium);
    }

    #[test]
    fn test_low_confidence_complex() {
        let validation =
            make_validation(8, vec!["loft", "shell", "cut", "union", "fuse"]);
        let result = assess_confidence(&validation, None);
        // base = 20, 0 cookbook matches + 5 distinct ops → -10 → 10
        assert!(result.score < 40, "score {} should be < 40", result.score);
        assert_eq!(result.level, ConfidenceLevel::Low);
    }

    #[test]
    fn test_cookbook_match_shell_recipe() {
        let ops = vec!["shell".to_string(), "extrude".to_string()];
        let cookbook = make_cookbook();
        let matches = match_cookbook(&ops, "make a hollow box", &cookbook);
        assert!(
            !matches.is_empty(),
            "should match at least one cookbook entry"
        );
        assert!(
            matches.iter().any(|m| m.title.contains("shell") || m.title.contains("Hollow")),
            "should match the shell recipe"
        );
    }

    #[test]
    fn test_cookbook_match_none() {
        // Operations that don't match any cookbook entry well
        let ops = vec!["chamfer".to_string(), "intersect".to_string()];
        let cookbook = make_cookbook();
        let matches = match_cookbook(&ops, "", &cookbook);
        // chamfer + intersect don't appear in any of our test cookbook entries
        assert!(
            matches.is_empty(),
            "should have no matches for novel combination, got {:?}",
            matches
        );
    }

    #[test]
    fn test_score_clamped_0_100() {
        // Very low risk + good cookbook = should not exceed 100
        let validation = make_validation(0, vec!["extrude", "shell"]);
        let cookbook = make_cookbook();
        let result = assess_confidence(&validation, Some(&cookbook));
        assert!(result.score <= 100, "score {} should be <= 100", result.score);

        // Very high risk + no cookbook + many ops = should not go below 0
        let validation2 =
            make_validation(10, vec!["loft", "shell", "sweep", "revolve", "cut"]);
        let result2 = assess_confidence(&validation2, None);
        assert!(result2.score >= 0, "score should not be negative");
        assert!(result2.score <= 100);
    }

    #[test]
    fn test_warnings_loft_shell() {
        let validation = make_validation(5, vec!["loft", "shell"]);
        let result = assess_confidence(&validation, None);
        assert!(
            result.warnings.iter().any(|w| w.contains("loft") && w.contains("shell")),
            "should warn about loft + shell"
        );
    }

    #[test]
    fn test_warnings_novel_combination() {
        let validation =
            make_validation(3, vec!["chamfer", "intersect", "fillet"]);
        let result = assess_confidence(&validation, None);
        assert!(
            result.warnings.iter().any(|w| w.contains("Novel operation")),
            "should warn about novel combination"
        );
    }

    #[test]
    fn test_warnings_high_complexity() {
        let validation = make_validation(
            3,
            vec!["loft", "shell", "cut", "union", "fillet"],
        );
        let result = assess_confidence(&validation, None);
        assert!(
            result.warnings.iter().any(|w| w.contains("High complexity")),
            "should warn about high complexity"
        );
    }

    #[test]
    fn test_match_cookbook_capped_at_3() {
        // Create many matching entries
        let ops = vec!["extrude".to_string(), "shell".to_string(), "cut".to_string()];
        let mut cookbook = Vec::new();
        for i in 0..10 {
            cookbook.push(CookbookEntry {
                title: format!("Recipe {} with extrude and shell and cut", i),
                description: Some("Uses extrude, shell, cut".to_string()),
                code: "import cadquery as cq\nresult = cq.Workplane('XY').box(1,1,1)".to_string(),
            });
        }
        let matches = match_cookbook(&ops, "", &cookbook);
        assert!(
            matches.len() <= 3,
            "should cap at 3 matches, got {}",
            matches.len()
        );
    }
}
