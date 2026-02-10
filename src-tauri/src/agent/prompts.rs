use crate::agent::rules::AgentRules;

/// Convert a snake_case key name into a Title Case heading.
fn format_category_name(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    upper + chars.as_str()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Recursively render a `serde_yaml::Value` into indented bullet-point text.
fn render_yaml_value(prompt: &mut String, value: &serde_yaml::Value, depth: usize) {
    let indent = "  ".repeat(depth);
    match value {
        serde_yaml::Value::Mapping(map) => {
            for (k, v) in map {
                if let serde_yaml::Value::String(key) = k {
                    let heading = format_category_name(key);
                    prompt.push_str(&format!("{}- **{}**", indent, heading));
                    match v {
                        serde_yaml::Value::String(s) => {
                            prompt.push_str(&format!(": {}\n", s));
                        }
                        serde_yaml::Value::Number(n) => {
                            prompt.push_str(&format!(": {}\n", n));
                        }
                        serde_yaml::Value::Bool(b) => {
                            prompt.push_str(&format!(": {}\n", b));
                        }
                        _ => {
                            prompt.push('\n');
                            render_yaml_value(prompt, v, depth + 1);
                        }
                    }
                }
            }
        }
        serde_yaml::Value::Sequence(seq) => {
            for item in seq {
                match item {
                    serde_yaml::Value::String(s) => {
                        prompt.push_str(&format!("{}- {}\n", indent, s));
                    }
                    _ => {
                        render_yaml_value(prompt, item, depth);
                    }
                }
            }
        }
        serde_yaml::Value::String(s) => {
            prompt.push_str(&format!("{}{}\n", indent, s));
        }
        serde_yaml::Value::Number(n) => {
            prompt.push_str(&format!("{}{}\n", indent, n));
        }
        serde_yaml::Value::Bool(b) => {
            prompt.push_str(&format!("{}{}\n", indent, b));
        }
        serde_yaml::Value::Null => {}
        serde_yaml::Value::Tagged(tagged) => {
            render_yaml_value(prompt, &tagged.value, depth);
        }
    }
}

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
            prompt.push_str(&format!("### {}\n", format_category_name(category)));
            for rule in rules_list {
                prompt.push_str(&format!("- {}\n", rule));
            }
        }
        prompt.push('\n');
    }

    // -- Design Thinking (mandatory pre-generation reasoning) --
    if let Some(ref dt) = rules.design_thinking {
        prompt.push_str("## Design Thinking (Required Before Code)\n");
        for (category, items) in dt {
            prompt.push_str(&format!("### {}\n", format_category_name(category)));
            for item in items {
                prompt.push_str(&format!("- {}\n", item));
            }
        }
        prompt.push('\n');
    }

    // -- Capabilities & Limitations --
    if let Some(ref caps) = rules.capabilities {
        prompt.push_str("## Capabilities & Limitations\n");
        for (category, items) in caps {
            prompt.push_str(&format!("### {}\n", format_category_name(category)));
            for item in items {
                prompt.push_str(&format!("- {}\n", item));
            }
        }
        prompt.push('\n');
    }

    // -- Advanced Techniques --
    if let Some(ref techniques) = rules.advanced_techniques {
        prompt.push_str("## Advanced Techniques\n");
        for (category, items) in techniques {
            prompt.push_str(&format!("### {}\n", format_category_name(category)));
            for item in items {
                prompt.push_str(&format!("- {}\n", item));
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

    // -- Validation checks --
    if let Some(ref validation) = rules.validation {
        prompt.push_str("## Validation Checks\n");
        if let Some(ref pre) = validation.pre_generation {
            prompt.push_str("### Pre-Generation\n");
            for item in pre {
                if let Some(map) = item.as_mapping() {
                    if let Some(check) = map.get(&serde_yaml::Value::String("check".into())) {
                        if let Some(check_str) = check.as_str() {
                            prompt.push_str(&format!(
                                "- **{}**",
                                format_category_name(check_str)
                            ));
                        }
                    }
                    if let Some(rule) = map.get(&serde_yaml::Value::String("rule".into())) {
                        if let Some(rule_str) = rule.as_str() {
                            prompt.push_str(&format!(": {}", rule_str));
                        }
                    }
                    prompt.push('\n');
                }
            }
        }
        if let Some(ref post) = validation.post_generation {
            prompt.push_str("### Post-Generation\n");
            for item in post {
                if let Some(map) = item.as_mapping() {
                    if let Some(check) = map.get(&serde_yaml::Value::String("check".into())) {
                        if let Some(check_str) = check.as_str() {
                            prompt.push_str(&format!(
                                "- **{}**",
                                format_category_name(check_str)
                            ));
                        }
                    }
                    if let Some(rule) = map.get(&serde_yaml::Value::String("rule".into())) {
                        if let Some(rule_str) = rule.as_str() {
                            prompt.push_str(&format!(": {}", rule_str));
                        }
                    }
                    prompt.push('\n');
                }
            }
        }
        prompt.push('\n');
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

    // -- Anti-Patterns (common mistakes to avoid) --
    if let Some(ref anti_patterns) = rules.anti_patterns {
        prompt.push_str("## Common Anti-Patterns — Mistakes to Avoid\n");
        prompt.push_str("These are common CadQuery mistakes. Study the wrong code, understand why it fails, and use the correct approach instead.\n\n");
        for (i, entry) in anti_patterns.iter().enumerate() {
            prompt.push_str(&format!("### Anti-Pattern {}: {}\n", i + 1, entry.title));
            prompt.push_str("**Wrong:**\n```python\n");
            prompt.push_str(&entry.wrong_code);
            if !entry.wrong_code.ends_with('\n') {
                prompt.push('\n');
            }
            prompt.push_str("```\n");
            prompt.push_str(&format!("**Error:** {}\n", entry.error_message));
            prompt.push_str(&format!("**Why:** {}\n", entry.explanation));
            prompt.push_str("**Correct:**\n```python\n");
            prompt.push_str(&entry.correct_code);
            if !entry.correct_code.ends_with('\n') {
                prompt.push('\n');
            }
            prompt.push_str("```\n\n");
        }
    }

    // -- Manufacturing awareness --
    if let Some(ref mfg) = rules.manufacturing {
        prompt.push_str("## Manufacturing Awareness\n");
        render_yaml_value(&mut prompt, mfg, 0);
        prompt.push('\n');
    }

    // -- Error handling rules --
    if let Some(ref on_err) = rules.on_error {
        prompt.push_str("## Error Handling\n");
        for (category, steps) in on_err {
            prompt.push_str(&format!("### {}\n", format_category_name(category)));
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
            prompt.push_str(&format!("### {}\n", format_category_name(category)));
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── format_category_name ───────────────────────────────────────────

    #[test]
    fn test_format_category_name_simple() {
        assert_eq!(format_category_name("hello"), "Hello");
    }

    #[test]
    fn test_format_category_name_snake_case() {
        assert_eq!(
            format_category_name("common_pitfalls"),
            "Common Pitfalls"
        );
    }

    #[test]
    fn test_format_category_name_multiple_words() {
        assert_eq!(
            format_category_name("strategy_for_complex_requests"),
            "Strategy For Complex Requests"
        );
    }

    #[test]
    fn test_format_category_name_empty() {
        assert_eq!(format_category_name(""), "");
    }

    // ── render_yaml_value ──────────────────────────────────────────────

    #[test]
    fn test_render_yaml_value_string() {
        let val = serde_yaml::Value::String("hello world".into());
        let mut out = String::new();
        render_yaml_value(&mut out, &val, 0);
        assert_eq!(out, "hello world\n");
    }

    #[test]
    fn test_render_yaml_value_number() {
        let val = serde_yaml::Value::Number(serde_yaml::Number::from(42));
        let mut out = String::new();
        render_yaml_value(&mut out, &val, 0);
        assert_eq!(out, "42\n");
    }

    #[test]
    fn test_render_yaml_value_mapping() {
        let yaml = "process: CNC Milling\nmin_wall: 1.0";
        let val: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let mut out = String::new();
        render_yaml_value(&mut out, &val, 0);
        assert!(out.contains("**Process**"));
        assert!(out.contains("CNC Milling"));
        assert!(out.contains("**Min Wall**"));
    }

    #[test]
    fn test_render_yaml_value_sequence() {
        let yaml = "- item one\n- item two";
        let val: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let mut out = String::new();
        render_yaml_value(&mut out, &val, 0);
        assert!(out.contains("- item one\n"));
        assert!(out.contains("- item two\n"));
    }

    #[test]
    fn test_render_yaml_value_nested() {
        let yaml = "outer:\n  inner_key: inner_value";
        let val: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let mut out = String::new();
        render_yaml_value(&mut out, &val, 0);
        assert!(out.contains("**Outer**"));
        assert!(out.contains("**Inner Key**"));
        assert!(out.contains("inner_value"));
    }

    #[test]
    fn test_render_yaml_value_bool() {
        let val = serde_yaml::Value::Bool(true);
        let mut out = String::new();
        render_yaml_value(&mut out, &val, 0);
        assert_eq!(out, "true\n");
    }

    #[test]
    fn test_render_yaml_value_null() {
        let val = serde_yaml::Value::Null;
        let mut out = String::new();
        render_yaml_value(&mut out, &val, 0);
        assert_eq!(out, "");
    }

    #[test]
    fn test_render_yaml_value_depth_indentation() {
        let val = serde_yaml::Value::String("indented".into());
        let mut out = String::new();
        render_yaml_value(&mut out, &val, 2);
        assert_eq!(out, "    indented\n");
    }

    // ── build_system_prompt — new sections present ─────────────────────

    #[test]
    fn test_prompt_contains_design_thinking_section() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(
            prompt.contains("## Design Thinking"),
            "prompt should have design thinking section"
        );
        assert!(prompt.contains("Mandatory Before Code"));
        assert!(prompt.contains("For Organic Shapes"));
        assert!(prompt.contains("For Complex Objects"));
    }

    #[test]
    fn test_prompt_contains_capabilities_section() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(
            prompt.contains("## Capabilities & Limitations"),
            "prompt should have capabilities section"
        );
    }

    #[test]
    fn test_prompt_contains_advanced_techniques_section() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(
            prompt.contains("## Advanced Techniques"),
            "prompt should have advanced techniques section"
        );
    }

    #[test]
    fn test_prompt_contains_validation_section() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(
            prompt.contains("## Validation Checks"),
            "prompt should have validation section"
        );
        assert!(prompt.contains("### Pre-Generation"));
        assert!(prompt.contains("### Post-Generation"));
    }

    #[test]
    fn test_prompt_contains_manufacturing_section() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(
            prompt.contains("## Manufacturing Awareness"),
            "prompt should have manufacturing section"
        );
    }

    #[test]
    fn test_prompt_capabilities_content() {
        let prompt = build_system_prompt_for_preset(None);
        // Check that the capabilities content is actually rendered
        assert!(prompt.contains("organic"));
        assert!(prompt.contains("NURBS"));
        assert!(prompt.contains("Prefer simpler geometry"));
    }

    #[test]
    fn test_prompt_advanced_techniques_content() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(prompt.contains("revolve()"));
        assert!(prompt.contains("sweep()"));
        assert!(prompt.contains("loft()"));
        assert!(prompt.contains("Fillet radius larger than edge"));
    }

    #[test]
    fn test_prompt_validation_content() {
        let prompt = build_system_prompt_for_preset(None);
        // Validation checks should be rendered with bold check names
        assert!(prompt.contains("**Dimensions Realistic**"));
        assert!(prompt.contains("**Mesh Watertight**"));
    }

    #[test]
    fn test_prompt_manufacturing_content() {
        let prompt = build_system_prompt_for_preset(None);
        // Manufacturing data from default.yaml
        assert!(prompt.contains("3d Printing") || prompt.contains("3D") || prompt.contains("3d_printing"));
    }

    // ── Existing sections still present ────────────────────────────────

    #[test]
    fn test_prompt_still_has_code_requirements() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(prompt.contains("## Code Requirements"));
        assert!(prompt.contains("import cadquery as cq"));
    }

    #[test]
    fn test_prompt_still_has_coordinate_system() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(prompt.contains("## Coordinate System"));
    }

    #[test]
    fn test_prompt_still_has_spatial_rules() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(prompt.contains("## Spatial Rules"));
    }

    #[test]
    fn test_prompt_still_has_cookbook() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(prompt.contains("## CadQuery Cookbook"));
        assert!(prompt.contains("### Recipe 1:"));
        // New recipes should also be present
        assert!(prompt.contains("Revolve"));
    }

    #[test]
    fn test_prompt_still_has_error_handling() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(prompt.contains("## Error Handling"));
    }

    #[test]
    fn test_prompt_still_has_response_format() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(prompt.contains("## Response Format"));
    }

    #[test]
    fn test_prompt_still_has_code_style() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(prompt.contains("## Code Style"));
    }

    // ── All 3 presets produce valid prompts ─────────────────────────────

    #[test]
    fn test_all_presets_produce_prompt_with_new_sections() {
        for preset in &[None, Some("3d-printing"), Some("cnc")] {
            let prompt = build_system_prompt_for_preset(*preset);
            assert!(
                prompt.contains("## Design Thinking"),
                "preset {:?} missing design thinking",
                preset
            );
            assert!(
                prompt.contains("## Capabilities & Limitations"),
                "preset {:?} missing capabilities",
                preset
            );
            assert!(
                prompt.contains("## Advanced Techniques"),
                "preset {:?} missing advanced techniques",
                preset
            );
            assert!(
                prompt.contains("## Validation Checks"),
                "preset {:?} missing validation",
                preset
            );
            assert!(
                prompt.contains("## Manufacturing Awareness"),
                "preset {:?} missing manufacturing",
                preset
            );
            assert!(
                prompt.contains("## CadQuery Cookbook"),
                "preset {:?} missing cookbook",
                preset
            );
        }
    }

    // ── Anti-Patterns in prompt ──────────────────────────────────────────

    #[test]
    fn test_prompt_contains_anti_patterns_section() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(
            prompt.contains("## Common Anti-Patterns"),
            "prompt should have anti-patterns section"
        );
        assert!(prompt.contains("Mistakes to Avoid"));
    }

    #[test]
    fn test_prompt_anti_patterns_content() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(prompt.contains("### Anti-Pattern 1:"));
        assert!(prompt.contains("Fillet before boolean"));
        assert!(prompt.contains("translate() wrong signature"));
        assert!(prompt.contains("Hole on wrong face"));
        assert!(prompt.contains("**Wrong:**"));
        assert!(prompt.contains("**Error:**"));
        assert!(prompt.contains("**Why:**"));
        assert!(prompt.contains("**Correct:**"));
    }

    #[test]
    fn test_all_presets_have_anti_patterns_in_prompt() {
        for preset in &[None, Some("3d-printing"), Some("cnc")] {
            let prompt = build_system_prompt_for_preset(*preset);
            assert!(
                prompt.contains("## Common Anti-Patterns"),
                "preset {:?} missing anti-patterns section",
                preset
            );
        }
    }

    // ── Section ordering ───────────────────────────────────────────────

    #[test]
    fn test_prompt_section_order() {
        let prompt = build_system_prompt_for_preset(None);
        let dt_pos = prompt.find("## Design Thinking").unwrap();
        let caps_pos = prompt.find("## Capabilities & Limitations").unwrap();
        let tech_pos = prompt.find("## Advanced Techniques").unwrap();
        let style_pos = prompt.find("## Code Style").unwrap();
        let valid_pos = prompt.find("## Validation Checks").unwrap();
        let cook_pos = prompt.find("## CadQuery Cookbook").unwrap();
        let ap_pos = prompt.find("## Common Anti-Patterns").unwrap();
        let mfg_pos = prompt.find("## Manufacturing Awareness").unwrap();
        let resp_pos = prompt.find("## Response Format").unwrap();

        // Design Thinking before Capabilities
        assert!(dt_pos < caps_pos, "Design Thinking should come before Capabilities");
        // Capabilities before Advanced Techniques
        assert!(caps_pos < tech_pos, "Capabilities should come before Advanced Techniques");
        // Advanced Techniques before Code Style
        assert!(tech_pos < style_pos, "Advanced Techniques should come before Code Style");
        // Validation after Code Style
        assert!(style_pos < valid_pos, "Code Style should come before Validation");
        // Cookbook after Validation
        assert!(valid_pos < cook_pos, "Validation should come before Cookbook");
        // Anti-Patterns after Cookbook
        assert!(cook_pos < ap_pos, "Cookbook should come before Anti-Patterns");
        // Manufacturing after Anti-Patterns
        assert!(ap_pos < mfg_pos, "Anti-Patterns should come before Manufacturing");
        // Response Format last
        assert!(mfg_pos < resp_pos, "Manufacturing should come before Response Format");
    }

    // ── default_empty produces minimal prompt ──────────────────────────

    #[test]
    fn test_empty_rules_produce_minimal_prompt() {
        let rules = AgentRules::default_empty();
        let prompt = build_system_prompt(&rules);
        // Should have the hard-coded intro and requirements
        assert!(prompt.contains("You are a CAD AI assistant"));
        assert!(prompt.contains("## Code Requirements"));
        // Should NOT have optional sections
        assert!(!prompt.contains("## Design Thinking"));
        assert!(!prompt.contains("## Capabilities"));
        assert!(!prompt.contains("## Advanced Techniques"));
        assert!(!prompt.contains("## Validation Checks"));
        assert!(!prompt.contains("## Manufacturing Awareness"));
        assert!(!prompt.contains("## Common Anti-Patterns"));
    }

    // ── Category name formatting in spatial rules ──────────────────────

    #[test]
    fn test_spatial_rules_use_formatted_names() {
        let prompt = build_system_prompt_for_preset(None);
        // spatial_rules keys like "boolean_cut" should appear as "Boolean Cut"
        assert!(
            prompt.contains("Boolean Cut") || prompt.contains("boolean_cut"),
            "spatial rules should have formatted category names"
        );
    }
}
