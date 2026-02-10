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

    // -- Design Patterns (parameterized templates) --
    if let Some(ref patterns) = rules.design_patterns {
        prompt.push_str("## Design Patterns — Parameterized Templates\n");
        prompt.push_str("These are higher-level templates for common object categories. ");
        prompt.push_str(
            "Use as starting points and customize parameters to match the user's request.\n\n",
        );
        for (i, entry) in patterns.iter().enumerate() {
            prompt.push_str(&format!("### Pattern {}: {}\n", i + 1, entry.name));
            prompt.push_str(&format!("{}\n", entry.description));
            prompt.push_str(&format!("**Keywords:** {}\n", entry.keywords.join(", ")));
            prompt.push_str("**Parameters:**\n");
            for p in &entry.parameters {
                prompt.push_str(&format!("- {}\n", p));
            }
            prompt.push_str("**Base code:**\n```python\n");
            prompt.push_str(&entry.base_code);
            if !entry.base_code.ends_with('\n') {
                prompt.push('\n');
            }
            prompt.push_str("```\n");
            prompt.push_str("**Variants:**\n");
            for v in &entry.variants {
                prompt.push_str(&format!("- {}\n", v));
            }
            prompt.push_str("**Gotchas:**\n");
            for g in &entry.gotchas {
                prompt.push_str(&format!("- {}\n", g));
            }
            prompt.push('\n');
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

    // -- Operation Interactions (cross-operation reasoning rules) --
    if let Some(ref interactions) = rules.operation_interactions {
        prompt.push_str("## Operation Interactions — Cross-Operation Reasoning Rules\n");
        prompt.push_str("CadQuery operations interact in subtle ways. ");
        prompt.push_str("Follow these rules when planning operation sequences.\n\n");
        for (pair_name, rules_list) in interactions {
            prompt.push_str(&format!("### {}\n", format_category_name(pair_name)));
            for rule in rules_list {
                prompt.push_str(&format!("- {}\n", rule));
            }
            prompt.push('\n');
        }
    }

    // -- API Quick-Reference --
    if let Some(ref api_ref) = rules.api_reference {
        prompt.push_str("## CadQuery API Quick-Reference\n");
        prompt.push_str("Compact reference for error-prone operations.\n\n");
        for entry in api_ref {
            prompt.push_str(&format!("### `{}`\n", entry.operation));
            prompt.push_str(&format!("**Signature:** `{}`\n", entry.signature));
            prompt.push_str(&format!("**Returns:** {}\n", entry.returns));
            prompt.push_str("**Key params:**\n");
            for p in &entry.params {
                prompt.push_str(&format!("- {}\n", p));
            }
            prompt.push_str("**Gotchas:**\n");
            for g in &entry.gotchas {
                prompt.push_str(&format!("- {}\n", g));
            }
            prompt.push('\n');
        }
    }

    // -- Dimension Tables (real-world reference dimensions) --
    if let Some(ref dim_tables) = rules.dimension_tables {
        prompt.push_str("## Real-World Dimension Tables\n");
        prompt.push_str("Use these dimensions instead of guessing. All values in mm.\n\n");
        for entry in dim_tables {
            prompt.push_str(&format!("### {}\n", entry.category));
            prompt.push_str(&format!("{}\n", entry.description));
            for item in &entry.data {
                prompt.push_str(&format!("- {}\n", item));
            }
            prompt.push('\n');
        }
    }

    // -- Dimension Estimation Guidance --
    if let Some(ref guidance) = rules.dimension_guidance {
        prompt.push_str("## Dimension Estimation Guidance\n");
        prompt.push_str("Follow these rules when choosing dimensions.\n\n");
        for (category, items) in guidance {
            prompt.push_str(&format!("### {}\n", format_category_name(category)));
            for item in items {
                prompt.push_str(&format!("- {}\n", item));
            }
            prompt.push('\n');
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

    // -- Failure Prevention (proactive rules) --
    if let Some(ref fp) = rules.failure_prevention {
        prompt.push_str("## Failure Prevention — Proactive Rules\n");
        prompt.push_str("Follow these rules BEFORE generating code to avoid common CadQuery failures.\n\n");
        for (category, items) in fp {
            prompt.push_str(&format!("### {}\n", format_category_name(category)));
            for item in items {
                prompt.push_str(&format!("- {}\n", item));
            }
            prompt.push('\n');
        }
    }

    // -- Few-Shot Examples (design-to-code workflow demonstrations) --
    if let Some(ref examples) = rules.few_shot_examples {
        prompt.push_str("## Few-Shot Examples: Design-to-Code Workflow\n");
        prompt.push_str(
            "These examples demonstrate the complete workflow from user request to working code.\n\n",
        );
        for (i, ex) in examples.iter().enumerate() {
            prompt.push_str(&format!("### Example {}\n", i + 1));
            prompt.push_str(&format!("**User request:** \"{}\"\n", ex.user_request));
            prompt.push_str("**Design plan:**\n");
            for line in ex.design_plan.lines() {
                if !line.trim().is_empty() {
                    prompt.push_str(&format!("- {}\n", line.trim()));
                }
            }
            prompt.push_str("**Code:**\n```python\n");
            prompt.push_str(&ex.code);
            if !ex.code.ends_with('\n') {
                prompt.push('\n');
            }
            prompt.push_str("```\n\n");
        }
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

    // -- Output structure (always present) --
    prompt.push_str("\n## Output Structure\n");
    prompt.push_str("When generating CadQuery code, wrap it in XML-style tags:\n\n");
    prompt.push_str("<CODE>\nimport cadquery as cq\n# ... your code ...\nresult = ...\n</CODE>\n\n");
    prompt.push_str("You may also use ```python fences inside or outside the tags.\n");
    prompt.push_str("The <CODE> tags help the system reliably extract your code.\n");

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
            assert!(
                prompt.contains("## Design Patterns"),
                "preset {:?} missing design patterns",
                preset
            );
            assert!(
                prompt.contains("## CadQuery API Quick-Reference"),
                "preset {:?} missing API reference",
                preset
            );
            assert!(
                prompt.contains("## Real-World Dimension Tables"),
                "preset {:?} missing dimension tables",
                preset
            );
            assert!(
                prompt.contains("## Few-Shot Examples: Design-to-Code Workflow"),
                "preset {:?} missing few-shot examples",
                preset
            );
            assert!(
                prompt.contains("## Failure Prevention"),
                "preset {:?} missing failure prevention",
                preset
            );
            assert!(
                prompt.contains("## Operation Interactions"),
                "preset {:?} missing operation interactions",
                preset
            );
        }
    }

    // ── Design Patterns in prompt ────────────────────────────────────────

    #[test]
    fn test_prompt_contains_design_patterns_section() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(
            prompt.contains("## Design Patterns"),
            "prompt should have design patterns section"
        );
        assert!(prompt.contains("Parameterized Templates"));
    }

    #[test]
    fn test_prompt_design_patterns_content() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(prompt.contains("Enclosure"), "missing Enclosure pattern");
        assert!(prompt.contains("Shaft"), "missing Shaft pattern");
        assert!(prompt.contains("Rotational"), "missing Rotational pattern");
        assert!(prompt.contains("Plate"), "missing Plate pattern");
        assert!(prompt.contains("Tube"), "missing Tube pattern");
        assert!(prompt.contains("Spring"), "missing Spring pattern");
        assert!(prompt.contains("Gear"), "missing Gear pattern");
        assert!(prompt.contains("**Keywords:**"));
        assert!(prompt.contains("**Parameters:**"));
        assert!(prompt.contains("**Base code:**"));
        assert!(prompt.contains("**Variants:**"));
        assert!(prompt.contains("**Gotchas:**"));
    }

    #[test]
    fn test_all_presets_have_design_patterns_in_prompt() {
        for preset in &[None, Some("3d-printing"), Some("cnc")] {
            let prompt = build_system_prompt_for_preset(*preset);
            assert!(
                prompt.contains("## Design Patterns"),
                "preset {:?} missing design patterns section",
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

    // ── API Reference in prompt ──────────────────────────────────────────

    #[test]
    fn test_prompt_contains_api_reference_section() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(
            prompt.contains("## CadQuery API Quick-Reference"),
            "prompt should have API reference section"
        );
        assert!(prompt.contains("Compact reference for error-prone operations."));
    }

    #[test]
    fn test_prompt_api_reference_content() {
        let prompt = build_system_prompt_for_preset(None);
        // Check operation names are rendered
        assert!(prompt.contains("### `loft()`"));
        assert!(prompt.contains("### `sweep()`"));
        assert!(prompt.contains("### `revolve()`"));
        assert!(prompt.contains("### `shell()`"));
        assert!(prompt.contains("### `Selector strings`"));
        assert!(prompt.contains("### `Workplane constructor & offsets`"));
        assert!(prompt.contains("### `pushPoints / rarray / polarArray`"));
        assert!(prompt.contains("### `.tag() / .faces(tag=)`"));
        // Check formatting markers
        assert!(prompt.contains("**Signature:**"));
        assert!(prompt.contains("**Returns:**"));
        assert!(prompt.contains("**Key params:**"));
        assert!(prompt.contains("**Gotchas:**"));
    }

    #[test]
    fn test_all_presets_have_api_reference_in_prompt() {
        for preset in &[None, Some("3d-printing"), Some("cnc")] {
            let prompt = build_system_prompt_for_preset(*preset);
            assert!(
                prompt.contains("## CadQuery API Quick-Reference"),
                "preset {:?} missing API reference section",
                preset
            );
        }
    }

    // ── Dimension Tables in prompt ─────────────────────────────────────

    #[test]
    fn test_prompt_contains_dimension_tables_section() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(
            prompt.contains("## Real-World Dimension Tables"),
            "prompt should have dimension tables section"
        );
        assert!(prompt.contains("Use these dimensions instead of guessing."));
    }

    #[test]
    fn test_prompt_dimension_tables_content() {
        let prompt = build_system_prompt_for_preset(None);
        // Spot-check representative data from each category
        assert!(prompt.contains("M6: shaft=6.0"), "missing M6 fastener data");
        assert!(prompt.contains("Raspberry Pi"), "missing Raspberry Pi data");
        assert!(prompt.contains("608"), "missing 608 bearing data");
        assert!(prompt.contains("Credit card"), "missing credit card data");
        assert!(prompt.contains("H7"), "missing H7 tolerance data");
        assert!(prompt.contains("Pin in hole"), "missing pin-in-hole data");
        assert!(prompt.contains("Aluminum"), "missing aluminum bend data");
    }

    #[test]
    fn test_all_presets_have_dimension_tables_in_prompt() {
        for preset in &[None, Some("3d-printing"), Some("cnc")] {
            let prompt = build_system_prompt_for_preset(*preset);
            assert!(
                prompt.contains("## Real-World Dimension Tables"),
                "preset {:?} missing dimension tables section",
                preset
            );
        }
    }

    // ── Few-Shot Examples in prompt ─────────────────────────────────────

    #[test]
    fn test_prompt_contains_few_shot_examples_section() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(
            prompt.contains("## Few-Shot Examples: Design-to-Code Workflow"),
            "prompt should have few-shot examples section"
        );
        assert!(prompt.contains("complete workflow from user request to working code"));
    }

    #[test]
    fn test_prompt_few_shot_examples_content() {
        let prompt = build_system_prompt_for_preset(None);
        // Spot-check all 5 examples are present
        assert!(prompt.contains("coffee mug"), "missing coffee mug example");
        assert!(prompt.contains("motor mount"), "missing motor mount example");
        assert!(prompt.contains("SD card"), "missing SD card example");
        assert!(prompt.contains("gear"), "missing gear example");
        assert!(prompt.contains("phone stand"), "missing phone stand example");
        // Check format markers
        assert!(prompt.contains("**User request:**"));
        assert!(prompt.contains("**Design plan:**"));
        assert!(prompt.contains("**Code:**"));
        assert!(prompt.contains("### Example 1"));
        assert!(prompt.contains("### Example 5"));
    }

    #[test]
    fn test_all_presets_have_few_shot_examples_in_prompt() {
        for preset in &[None, Some("3d-printing"), Some("cnc")] {
            let prompt = build_system_prompt_for_preset(*preset);
            assert!(
                prompt.contains("## Few-Shot Examples: Design-to-Code Workflow"),
                "preset {:?} missing few-shot examples section",
                preset
            );
        }
    }

    // ── Operation Interactions in prompt ───────────────────────────────────

    #[test]
    fn test_prompt_contains_operation_interactions_section() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(
            prompt.contains("## Operation Interactions — Cross-Operation Reasoning Rules"),
            "prompt should have operation interactions section"
        );
        assert!(prompt.contains("Follow these rules when planning operation sequences."));
    }

    #[test]
    fn test_prompt_operation_interactions_content() {
        let prompt = build_system_prompt_for_preset(None);
        // Spot-check category names (formatted from snake_case)
        assert!(prompt.contains("Fillet After Boolean"), "missing fillet_after_boolean category");
        assert!(prompt.contains("Shell After Fillet"), "missing shell_after_fillet category");
        assert!(prompt.contains("Loft Then Shell"), "missing loft_then_shell category");
        assert!(prompt.contains("Boolean Chain Limit"), "missing boolean_chain_limit category");
        assert!(prompt.contains("Extrude On Face"), "missing extrude_on_face category");
        assert!(prompt.contains("Sweep With Boolean"), "missing sweep_with_boolean category");
        assert!(prompt.contains("Revolve Then Cut"), "missing revolve_then_cut category");
        assert!(prompt.contains("Operation Ordering"), "missing operation_ordering category");
    }

    #[test]
    fn test_all_presets_have_operation_interactions_in_prompt() {
        for preset in &[None, Some("3d-printing"), Some("cnc")] {
            let prompt = build_system_prompt_for_preset(*preset);
            assert!(
                prompt.contains("## Operation Interactions"),
                "preset {:?} missing operation interactions section",
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
        let dp_pos = prompt.find("## Design Patterns").unwrap();
        let ap_pos = prompt.find("## Common Anti-Patterns").unwrap();
        let apiref_pos = prompt.find("## CadQuery API Quick-Reference").unwrap();
        let dimtab_pos = prompt.find("## Real-World Dimension Tables").unwrap();
        let dimguide_pos = prompt.find("## Dimension Estimation Guidance").unwrap();
        let mfg_pos = prompt.find("## Manufacturing Awareness").unwrap();
        let err_pos = prompt.find("## Error Handling").unwrap();
        let fse_pos = prompt.find("## Few-Shot Examples").unwrap();
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
        // Design Patterns after Cookbook
        assert!(cook_pos < dp_pos, "Cookbook should come before Design Patterns");
        // Anti-Patterns after Design Patterns
        assert!(dp_pos < ap_pos, "Design Patterns should come before Anti-Patterns");
        // Operation Interactions after Anti-Patterns
        let oi_pos = prompt.find("## Operation Interactions").unwrap();
        assert!(ap_pos < oi_pos, "Anti-Patterns should come before Operation Interactions");
        // API Reference after Operation Interactions
        assert!(oi_pos < apiref_pos, "Operation Interactions should come before API Reference");
        // Dimension Tables after API Reference
        assert!(apiref_pos < dimtab_pos, "API Reference should come before Dimension Tables");
        // Dimension Guidance after Dimension Tables
        assert!(dimtab_pos < dimguide_pos, "Dimension Tables should come before Dimension Guidance");
        // Manufacturing after Dimension Guidance
        assert!(dimguide_pos < mfg_pos, "Dimension Guidance should come before Manufacturing");
        // Error Handling after Manufacturing
        assert!(mfg_pos < err_pos, "Manufacturing should come before Error Handling");
        // Failure Prevention after Error Handling
        let fp_pos = prompt.find("## Failure Prevention").unwrap();
        assert!(err_pos < fp_pos, "Error Handling should come before Failure Prevention");
        // Few-Shot Examples after Failure Prevention
        assert!(fp_pos < fse_pos, "Failure Prevention should come before Few-Shot Examples");
        // Response Format last
        assert!(fse_pos < resp_pos, "Few-Shot Examples should come before Response Format");
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
        assert!(!prompt.contains("## Design Patterns"));
        assert!(!prompt.contains("## Common Anti-Patterns"));
        assert!(!prompt.contains("## CadQuery API Quick-Reference"));
        assert!(!prompt.contains("## Real-World Dimension Tables"));
        assert!(!prompt.contains("## Dimension Estimation Guidance"));
        assert!(!prompt.contains("## Failure Prevention"));
        assert!(!prompt.contains("## Few-Shot Examples"));
        assert!(!prompt.contains("## Operation Interactions"));
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

    // ── Dimension Estimation Guidance in prompt ─────────────────────────

    #[test]
    fn test_prompt_contains_dimension_guidance_section() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(
            prompt.contains("## Dimension Estimation Guidance"),
            "prompt should have dimension estimation guidance section"
        );
        assert!(prompt.contains("Follow these rules when choosing dimensions."));
    }

    #[test]
    fn test_all_presets_have_dimension_guidance_in_prompt() {
        for preset in &[None, Some("3d-printing"), Some("cnc")] {
            let prompt = build_system_prompt_for_preset(*preset);
            assert!(
                prompt.contains("## Dimension Estimation Guidance"),
                "preset {:?} missing dimension estimation guidance section",
                preset
            );
        }
    }

    #[test]
    fn test_prompt_dimension_guidance_content() {
        let prompt = build_system_prompt_for_preset(None);
        // Spot-check key content from each category
        assert!(prompt.contains("real-world objects"), "missing when_to_estimate content");
        assert!(prompt.contains("Tiny: < 20mm"), "missing size_classes content");
        assert!(prompt.contains("Human hand"), "missing scale_anchors content");
        assert!(prompt.contains("mug/cup"), "missing proportional_reasoning content");
        assert!(prompt.contains("Lid or cap"), "missing relative_sizing content");
    }

    #[test]
    fn test_prompt_no_never_assume_dimensions() {
        for preset in &[None, Some("3d-printing"), Some("cnc")] {
            let prompt = build_system_prompt_for_preset(*preset);
            assert!(
                !prompt.contains("Never assume dimensions"),
                "preset {:?} still has old 'Never assume dimensions' rule",
                preset
            );
        }
    }

    // ── Failure Prevention in prompt ──────────────────────────────────────

    #[test]
    fn test_prompt_contains_failure_prevention_section() {
        let prompt = build_system_prompt_for_preset(None);
        assert!(
            prompt.contains("## Failure Prevention — Proactive Rules"),
            "prompt should have failure prevention section"
        );
        assert!(prompt.contains("Follow these rules BEFORE generating code"));
    }

    #[test]
    fn test_prompt_failure_prevention_content() {
        let prompt = build_system_prompt_for_preset(None);
        // Spot-check each category
        assert!(prompt.contains("fillet() fails"), "missing self_diagnosis content");
        assert!(prompt.contains("about to use shell()"), "missing preemptive_warnings content");
        assert!(prompt.contains("loft() fails between two profiles"), "missing alternative_operations content");
        assert!(prompt.contains("exceeds 50 lines"), "missing complexity_assessment content");
        assert!(prompt.contains("Before outputting code, verify"), "missing pre_output_checklist content");
    }

    #[test]
    fn test_all_presets_have_failure_prevention_in_prompt() {
        for preset in &[None, Some("3d-printing"), Some("cnc")] {
            let prompt = build_system_prompt_for_preset(*preset);
            assert!(
                prompt.contains("## Failure Prevention"),
                "preset {:?} missing failure prevention section",
                preset
            );
        }
    }
}
