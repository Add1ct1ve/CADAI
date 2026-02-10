use crate::ai::message::ChatMessage;
use crate::ai::provider::AiProvider;
use crate::error::AppError;

/// The geometry design plan produced by the advisor before code generation.
#[derive(Debug, Clone)]
pub struct DesignPlan {
    pub text: String,
}

const GEOMETRY_ADVISOR_PROMPT: &str = r#"You are a CAD geometry planner. Your job is to analyze a user's request and produce a detailed geometric build plan BEFORE any code is written.

You must think carefully about what the object actually looks like and how to build it with CadQuery primitives. Do NOT write any code — describe the geometry.

## Your Output Format

### Object Analysis
Describe what this object looks like in the real world. What are its key visual features? What are its proportions? What makes it recognizable?

### CadQuery Approach
Which CadQuery primitives and operations best approximate each feature? Be specific:
- For axially symmetric shapes → revolve() with a spline/polyline profile
- For shapes that vary along a height → loft() between profiles at different heights
- For shapes with a constant cross-section along a path → sweep()
- For mechanical parts → boxes, cylinders, and boolean operations
- For organic curves → spline profiles with revolve/loft, generous fillets

### Build Plan
Number each step. Be specific about dimensions and positions:
1. Start with [base shape] — dimensions: X×Y×Z mm
2. Add [feature] using [operation] — positioned at (x, y, z)
3. Cut [opening] using [method] — dimensions and position
...

### Approximation Notes
What can't CadQuery do perfectly? What's the closest buildable shape? Where should fillets be applied to smooth transitions?

## Rules
- Think about CROSS-SECTIONS: describe the profile shape at key heights
- Think about PROPORTIONS: a helmet is roughly 200mm tall, a phone is ~150mm long, etc.
- Think about what makes the object RECOGNIZABLE — which features are essential vs decorative
- For organic shapes: plan by cross-section at multiple heights, then use loft or revolve
- For mechanical parts: plan by feature (base → holes → slots → fillets)
- Prefer approaches that are ROBUST in CadQuery (box+cylinder+booleans > complex lofts)
- If the request is simple (e.g. "a box" or "a cylinder"), keep the plan brief — 2-3 lines is fine
- NEVER write Python or CadQuery code — only describe geometry in plain English"#;

/// Call the AI to produce a geometry design plan for the user's request.
///
/// This is the "design-first" phase that runs before code generation,
/// giving the code generator concrete geometric instructions instead of
/// a vague natural-language description.
pub async fn plan_geometry(
    provider: Box<dyn AiProvider>,
    user_request: &str,
) -> Result<DesignPlan, AppError> {
    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: GEOMETRY_ADVISOR_PROMPT.to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: user_request.to_string(),
        },
    ];

    // Use complete (non-streaming) since the plan is relatively short
    // and we want the full text before proceeding to code generation.
    let plan_text = provider.complete(&messages, Some(2048)).await?;

    Ok(DesignPlan { text: plan_text })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geometry_advisor_prompt_content() {
        assert!(GEOMETRY_ADVISOR_PROMPT.contains("geometry planner"));
        assert!(GEOMETRY_ADVISOR_PROMPT.contains("Build Plan"));
        assert!(GEOMETRY_ADVISOR_PROMPT.contains("Object Analysis"));
        assert!(GEOMETRY_ADVISOR_PROMPT.contains("CadQuery Approach"));
        assert!(GEOMETRY_ADVISOR_PROMPT.contains("NEVER write Python"));
    }

    #[test]
    fn test_design_plan_struct() {
        let plan = DesignPlan {
            text: "Test plan".to_string(),
        };
        assert_eq!(plan.text, "Test plan");
    }
}
