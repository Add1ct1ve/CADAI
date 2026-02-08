use crate::agent::rules::AgentRules;

/// Build a system prompt for the CAD AI agent from the loaded agent rules.
pub fn build_system_prompt(rules: &AgentRules) -> String {
    let mut prompt = String::new();

    prompt.push_str("You are a CAD AI assistant that generates CadQuery (Python) code. ");
    prompt.push_str("You create 3D models based on user descriptions.\n\n");

    // -- Code requirements (hard-coded essentials + rules from YAML) --
    prompt.push_str("## Code Requirements\n");
    prompt.push_str("- Always import cadquery as cq\n");
    prompt.push_str("- The final result MUST be assigned to a variable named 'result'\n");
    prompt.push_str("- All dimensions are in millimeters\n");
    prompt.push_str("- Use CadQuery's fluent API (method chaining)\n");
    prompt.push_str("- Do NOT use show_object(), display(), or any GUI calls\n");
    prompt.push_str("- Do NOT read/write files or use any external resources\n\n");

    if let Some(ref reqs) = rules.code_requirements {
        if let Some(ref mandatory) = reqs.mandatory {
            prompt.push_str("### Mandatory\n");
            for rule in mandatory {
                prompt.push_str(&format!("- {}\n", rule));
            }
            prompt.push('\n');
        }
        if let Some(ref forbidden) = reqs.forbidden {
            prompt.push_str("### Forbidden\n");
            for rule in forbidden {
                prompt.push_str(&format!("- {}\n", rule));
            }
            prompt.push('\n');
        }
    }

    // -- Coordinate system --
    if let Some(ref cs) = rules.coordinate_system {
        prompt.push_str("## Coordinate System\n");
        if let Some(ref desc) = cs.description {
            prompt.push_str(&format!("{}\n", desc));
        }
        if let Some(ref x) = cs.x {
            if let (Some(ref dir), Some(ref pos)) = (&x.direction, &x.positive) {
                prompt.push_str(&format!("- X axis: {} (positive = {})\n", dir, pos));
            }
        }
        if let Some(ref y) = cs.y {
            if let (Some(ref dir), Some(ref pos)) = (&y.direction, &y.positive) {
                prompt.push_str(&format!("- Y axis: {} (positive = {})\n", dir, pos));
            }
        }
        if let Some(ref z) = cs.z {
            if let (Some(ref dir), Some(ref pos)) = (&z.direction, &z.positive) {
                prompt.push_str(&format!("- Z axis: {} (positive = {})\n", dir, pos));
            }
        }
        if let Some(ref origin) = cs.origin {
            prompt.push_str(&format!("- Origin: {}\n", origin));
        }
        prompt.push('\n');
    }

    // -- Spatial rules --
    if let Some(ref sr) = rules.spatial_rules {
        prompt.push_str("## Spatial Rules\n");
        for (category, rules_list) in sr {
            prompt.push_str(&format!("### {}\n", category));
            for rule in rules_list {
                prompt.push_str(&format!("- {}\n", rule));
            }
        }
        prompt.push('\n');
    }

    // -- Code style --
    if let Some(ref style) = rules.code_style {
        prompt.push_str("## Code Style\n");
        if let Some(ref naming) = style.naming {
            prompt.push_str("### Naming\n");
            for n in naming {
                prompt.push_str(&format!("- {}\n", n));
            }
        }
        if let Some(ref comments) = style.comments {
            prompt.push_str("### Comments\n");
            for c in comments {
                prompt.push_str(&format!("- {}\n", c));
            }
        }
        if let Some(ref organization) = style.organization {
            prompt.push_str("### Organization\n");
            for o in organization {
                prompt.push_str(&format!("- {}\n", o));
            }
        }
        if let Some(ref example) = style.example {
            prompt.push_str("\n### Example\n```python\n");
            prompt.push_str(example);
            if !example.ends_with('\n') {
                prompt.push('\n');
            }
            prompt.push_str("```\n\n");
        }
    }

    // -- Cookbook (concrete code recipes) --
    if let Some(ref cookbook) = rules.cookbook {
        prompt.push_str("## CadQuery Cookbook - Reference Patterns\n");
        prompt.push_str("Use these as reference for correct CadQuery API usage.\n\n");
        for (i, entry) in cookbook.iter().enumerate() {
            prompt.push_str(&format!("### Recipe {}: {}\n", i + 1, entry.title));
            if let Some(ref desc) = entry.description {
                prompt.push_str(&format!("{}\n", desc));
            }
            prompt.push_str("```python\n");
            prompt.push_str(&entry.code);
            if !entry.code.ends_with('\n') {
                prompt.push('\n');
            }
            prompt.push_str("```\n\n");
        }
    }

    // -- Error handling rules --
    if let Some(ref on_err) = rules.on_error {
        prompt.push_str("## Error Handling\n");
        for (category, steps) in on_err {
            prompt.push_str(&format!("### {}\n", category));
            for step in steps {
                prompt.push_str(&format!("- {}\n", step));
            }
        }
        prompt.push('\n');
    }

    // -- Response format --
    prompt.push_str("## Response Format\n");
    if let Some(ref rf) = rules.response_format {
        for (category, items) in rf {
            prompt.push_str(&format!("### {}\n", category));
            for item in items {
                prompt.push_str(&format!("- {}\n", item));
            }
        }
    } else {
        prompt.push_str("When generating code, wrap it in a ```python code block.\n");
        prompt.push_str("Always provide a brief description before the code.\n");
        prompt.push_str(
            "If the request is ambiguous, ask clarifying questions instead of guessing.\n",
        );
    }

    prompt
}

/// Build a system prompt for a specific preset (e.g. "3d-printing", "cnc", or None for default).
pub fn build_system_prompt_for_preset(preset_name: Option<&str>) -> String {
    let rules = AgentRules::from_preset(preset_name)
        .unwrap_or_else(|_| AgentRules::from_preset(None).unwrap());
    build_system_prompt(&rules)
}

/// Build a system prompt with default rules.
#[allow(dead_code)]
pub fn build_default_system_prompt() -> String {
    build_system_prompt_for_preset(None)
}
