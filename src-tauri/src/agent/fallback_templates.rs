use regex::Regex;

#[derive(Debug, Clone)]
pub struct FallbackTemplate {
    pub template_id: &'static str,
    pub reason: String,
    pub code: String,
}

#[derive(Debug, Clone)]
pub struct FallbackPlan {
    pub template_id: &'static str,
    pub reason: String,
    pub plan_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PartTemplateKind {
    Housing,
    Plate,
    SnapLidBox,
}

fn tokenize_words(input: &str) -> Vec<String> {
    let re = Regex::new(r"[A-Za-z0-9_]+").expect("valid token regex");
    re.find_iter(&input.to_lowercase())
        .map(|m| m.as_str().to_string())
        .collect()
}

fn has_token(tokens: &[String], token: &str) -> bool {
    tokens.iter().any(|t| t == token)
}

fn has_phrase(tokens: &[String], first: &str, second: &str) -> bool {
    tokens
        .windows(2)
        .any(|window| window[0] == first && window[1] == second)
}

fn score_template_kind(part_name: &str, description: &str) -> Option<PartTemplateKind> {
    let name_tokens = tokenize_words(part_name);
    let desc_tokens = tokenize_words(description);

    let explicit_housing_name = has_token(&name_tokens, "housing")
        || has_token(&name_tokens, "enclosure")
        || has_token(&name_tokens, "main_body")
        || has_token(&name_tokens, "body")
        || has_token(&name_tokens, "case");
    let explicit_plate_name = has_token(&name_tokens, "back_plate")
        || has_token(&name_tokens, "backplate")
        || has_phrase(&name_tokens, "back", "plate")
        || has_token(&name_tokens, "cover")
        || has_token(&name_tokens, "lid");

    let mut housing_score = 0_i32;
    if explicit_housing_name {
        housing_score += 9;
    }
    if has_token(&desc_tokens, "housing") || has_token(&desc_tokens, "enclosure") {
        housing_score += 3;
    }
    if has_token(&desc_tokens, "slot") || has_token(&desc_tokens, "slots") {
        housing_score += 2;
    }
    if has_token(&desc_tokens, "ledge")
        || has_token(&desc_tokens, "cavity")
        || has_token(&desc_tokens, "button")
    {
        housing_score += 1;
    }

    let mut plate_score = 0_i32;
    if explicit_plate_name {
        plate_score += 9;
    }
    if has_phrase(&desc_tokens, "back", "plate")
        || has_token(&desc_tokens, "backplate")
        || has_token(&desc_tokens, "cover")
        || has_token(&desc_tokens, "lid")
    {
        plate_score += 3;
    }
    if has_token(&desc_tokens, "lip") {
        plate_score += 2;
    }
    if has_token(&desc_tokens, "ridge")
        || has_token(&desc_tokens, "oring")
        || has_token(&desc_tokens, "o_ring")
    {
        plate_score += 2;
    }

    let mut snap_score = 0_i32;
    if has_token(&name_tokens, "snap_lid_box") {
        snap_score += 9;
    }
    if has_token(&desc_tokens, "snap") && has_token(&desc_tokens, "box") {
        snap_score += 4;
    }
    if has_token(&desc_tokens, "latch") || has_token(&desc_tokens, "detent") {
        snap_score += 1;
    }

    let mut scored = vec![
        (PartTemplateKind::Housing, housing_score),
        (PartTemplateKind::Plate, plate_score),
        (PartTemplateKind::SnapLidBox, snap_score),
    ];
    scored.sort_by(|a, b| b.1.cmp(&a.1));

    let (best_kind, best_score) = scored[0];
    if best_score < 3 {
        return None;
    }

    let second_score = scored[1].1;
    let ambiguous = best_score - second_score <= 1;
    if ambiguous && !explicit_housing_name && !explicit_plate_name {
        return None;
    }

    Some(best_kind)
}

fn parse_dim(text: &str, label: &str, default_val: f64) -> f64 {
    let pat = format!(
        r"(?i)\b{}\b[^0-9-]*(-?\d+(?:\.\d+)?)\s*mm",
        regex::escape(label)
    );
    Regex::new(&pat)
        .ok()
        .and_then(|re| re.captures(text))
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<f64>().ok())
        .filter(|v| *v > 0.0)
        .unwrap_or(default_val)
}

fn parse_compact_dims(text: &str) -> Option<[f64; 3]> {
    let re = Regex::new(
        r"(?i)(\d+(?:\.\d+)?)\s*[x×]\s*(\d+(?:\.\d+)?)\s*[x×]\s*(\d+(?:\.\d+)?)\s*(?:mm)?",
    )
    .unwrap();
    let caps = re.captures(text)?;
    let a = caps.get(1)?.as_str().parse::<f64>().ok()?;
    let b = caps.get(2)?.as_str().parse::<f64>().ok()?;
    let c = caps.get(3)?.as_str().parse::<f64>().ok()?;
    if a > 0.0 && b > 0.0 && c > 0.0 {
        Some([a, b, c])
    } else {
        None
    }
}

fn plate_with_lip_ridge_template(part_name: &str, description: &str) -> FallbackTemplate {
    let dims = parse_compact_dims(description).unwrap_or([28.0, 24.0, 1.5]);
    let plate_len = dims[0].max(6.0);
    let plate_wid = dims[1].max(6.0);
    let plate_thk = parse_dim(description, "thickness", dims[2]).max(0.8);
    let lip_height = parse_dim(description, "lip", 1.2).max(0.5);
    let ridge_height = 0.5_f64;

    let code = format!(
        r#"import cadquery as cq

# deterministic fallback template: plate_with_lip_ridge
PL = {plate_len:.3}
PW = {plate_wid:.3}
PT = {plate_thk:.3}
LIP_INSET = 2.0
LIP_THK = 1.5
LIP_H = {lip_height:.3}
RIDGE_W = 1.0
RIDGE_H = {ridge_height:.3}

plate = cq.Workplane("XY").box(PL, PW, PT, centered=(True, True, False))

lip_outer = (
    cq.Workplane("XY", origin=(0, 0, PT))
    .rect(max(PL - 2 * LIP_INSET, 2.0), max(PW - 2 * LIP_INSET, 2.0))
    .extrude(LIP_H)
)
lip_inner = (
    cq.Workplane("XY", origin=(0, 0, PT))
    .rect(max(PL - 2 * (LIP_INSET + LIP_THK), 1.0), max(PW - 2 * (LIP_INSET + LIP_THK), 1.0))
    .extrude(LIP_H)
)
lip = lip_outer.cut(lip_inner)

ridge_outer = (
    cq.Workplane("XY", origin=(0, 0, PT + LIP_H - RIDGE_H))
    .rect(max(PL - 2 * (LIP_INSET + 0.6), 1.0), max(PW - 2 * (LIP_INSET + 0.6), 1.0))
    .extrude(RIDGE_H)
)
ridge_inner = (
    cq.Workplane("XY", origin=(0, 0, PT + LIP_H - RIDGE_H))
    .rect(max(PL - 2 * (LIP_INSET + 0.6 + RIDGE_W), 0.8), max(PW - 2 * (LIP_INSET + 0.6 + RIDGE_W), 0.8))
    .extrude(RIDGE_H)
)
ridge = ridge_outer.cut(ridge_inner)

result = plate.union(lip).union(ridge)
"#
    );

    FallbackTemplate {
        template_id: "plate_with_lip_ridge",
        reason: format!(
            "deterministic fallback for '{}' due repeated topology/semantic failures",
            part_name
        ),
        code,
    }
}

fn simple_housing_with_slots_template(part_name: &str, description: &str) -> FallbackTemplate {
    let dims = parse_compact_dims(description).unwrap_or([42.0, 28.0, 7.5]);
    let length = parse_dim(description, "length", dims[0]).max(10.0);
    let width = parse_dim(description, "width", dims[1]).max(8.0);
    let height = parse_dim(description, "height", dims[2]).max(4.0);
    let wall = parse_dim(description, "wall", 1.8).clamp(0.8, 4.0);
    let top_thk = parse_dim(description, "top_thk", 1.5).clamp(0.8, (height - 1.0).max(1.0));
    let back_lip = parse_dim(description, "back_lip", 1.5).clamp(0.6, (height - 0.4).max(0.6));
    let oring_w = parse_dim(description, "oring_width", 1.2).clamp(0.6, 2.0);
    let oring_d = parse_dim(description, "oring_depth", 0.8).clamp(0.3, 1.5);
    let chamfer = 0.6_f64.min(wall * 0.3);
    let button_len =
        parse_dim(description, "button_length", 12.0).clamp(4.0, (length * 0.5).max(4.0));
    let button_wid =
        parse_dim(description, "button_width", 4.0).clamp(2.0, (height * 0.6).max(2.0));
    let button_off =
        parse_dim(description, "button_offset", 6.0).clamp(-length * 0.4, length * 0.4);
    let slot_w = parse_dim(description, "slot width", 20.0).clamp(2.0, width - 1.0);
    let slot_h = parse_dim(description, "slot height", 2.5).clamp(1.0, height - 0.5);
    let slot_d = parse_dim(description, "slot depth", 5.0).clamp(1.0, length * 0.45);

    let code = format!(
        r#"import cadquery as cq

# deterministic fallback template: simple_housing_with_slots
L = {length:.3}
W = {width:.3}
H = {height:.3}
WALL = {wall:.3}
TOP = {top_thk:.3}
BACK_LIP = {back_lip:.3}
ORING_W = {oring_w:.3}
ORING_D = {oring_d:.3}
CHAMFER = {chamfer:.3}
SLOT_W = {slot_w:.3}
SLOT_H = {slot_h:.3}
SLOT_D = {slot_d:.3}
BTN_L = {button_len:.3}
BTN_W = {button_wid:.3}
BTN_OFF = {button_off:.3}

# Single outer box (no stepped cap) — chamfer gives the finished look
outer = cq.Workplane("XY").box(L, W, H, centered=(True, True, False))

# Chamfer top edges for a polished appearance
try:
    outer = outer.edges(">Z").chamfer(CHAMFER)
except Exception:
    pass  # skip chamfer if geometry is too small

inner = cq.Workplane("XY", origin=(0, 0, BACK_LIP)).box(
    max(L - 2 * WALL, 1.0),
    max(W - 2 * WALL, 1.0),
    max(H - TOP - BACK_LIP, 1.0),
    centered=(True, True, False),
)
housing = outer.cut(inner)

# O-ring groove on ledge (rectangular ring cut)
g_outer_len = max(L - 2 * (WALL + 1.0), 2.0)
g_outer_wid = max(W - 2 * (WALL + 1.0), 2.0)
g_inner_len = max(g_outer_len - 2 * ORING_W, 1.0)
g_inner_wid = max(g_outer_wid - 2 * ORING_W, 1.0)
groove_outer = cq.Workplane("XY", origin=(0, 0, max(BACK_LIP - ORING_D, 0.0))).rect(g_outer_len, g_outer_wid).extrude(ORING_D)
groove_inner = cq.Workplane("XY", origin=(0, 0, max(BACK_LIP - ORING_D, 0.0))).rect(g_inner_len, g_inner_wid).extrude(ORING_D)
housing = housing.cut(groove_outer.cut(groove_inner))

# End slots — cut through the full wall at each end
slot = cq.Workplane("XY", origin=(L / 2 - SLOT_D / 2, 0, BACK_LIP)).box(
    SLOT_D, SLOT_W, SLOT_H, centered=(True, True, False)
)
housing = housing.cut(slot).cut(slot.mirror("YZ"))

# Side button indicator — 0.6mm recess for visibility
indicator = cq.Workplane("XY", origin=(BTN_OFF, W / 2 - 0.3, H * 0.5)).box(
    max(BTN_L, 1.0), 0.6, max(BTN_W, 1.0), centered=(True, True, True)
)
housing = housing.cut(indicator)

result = housing
"#
    );

    FallbackTemplate {
        template_id: "simple_housing_with_slots",
        reason: format!(
            "deterministic fallback for '{}' due repeated topology/semantic failures",
            part_name
        ),
        code,
    }
}

fn snap_lid_box_template(part_name: &str, description: &str) -> FallbackTemplate {
    let dims = parse_compact_dims(description).unwrap_or([60.0, 40.0, 25.0]);
    let length = dims[0].max(12.0);
    let width = dims[1].max(12.0);
    let height = dims[2].max(8.0);
    let wall = 2.0_f64;

    let code = format!(
        r#"import cadquery as cq

# deterministic fallback template: snap_lid_box
L = {length:.3}
W = {width:.3}
H = {height:.3}
WALL = {wall:.3}

outer = cq.Workplane("XY").box(L, W, H, centered=(True, True, False))
inner = cq.Workplane("XY", origin=(0, 0, WALL)).box(
    max(L - 2 * WALL, 1.0),
    max(W - 2 * WALL, 1.0),
    max(H - WALL, 1.0),
    centered=(True, True, False),
)
result = outer.cut(inner)
"#
    );

    FallbackTemplate {
        template_id: "snap_lid_box",
        reason: format!(
            "deterministic fallback for '{}' due repeated topology/semantic failures",
            part_name
        ),
        code,
    }
}

pub fn maybe_template_for_part(part_name: &str, description: &str) -> Option<FallbackTemplate> {
    match score_template_kind(part_name, description) {
        Some(PartTemplateKind::Housing) => {
            Some(simple_housing_with_slots_template(part_name, description))
        }
        Some(PartTemplateKind::Plate) => {
            Some(plate_with_lip_ridge_template(part_name, description))
        }
        Some(PartTemplateKind::SnapLidBox) => Some(snap_lid_box_template(part_name, description)),
        None => None,
    }
}

pub fn maybe_fallback_plan(user_request: &str) -> Option<FallbackPlan> {
    let lower = user_request.to_lowercase();

    if (lower.contains("housing") || lower.contains("enclosure"))
        && (lower.contains("back plate")
            || lower.contains("backplate")
            || lower.contains("separate"))
    {
        return Some(FallbackPlan {
            template_id: "enclosure_with_back_plate",
            reason: "planner failed repeatedly; switched to deterministic enclosure/back-plate plan".to_string(),
            plan_text: r#"### Object Analysis
- Compact enclosure body with a removable back plate.

### CadQuery Approach
- Primary robust path: build outer solid by `box`/`extrude`, create cavity by explicit inner-solid subtraction.
- Optional high-fidelity fallback: add guarded fillets/chamfers only after all booleans succeed.

### Build Plan
1. Build housing outer body from explicit dimensions with bottom at Z=0.
2. Subtract an inner solid to form cavity and preserve a back-plate ledge.
3. Add strap/slot/button features via explicit cutter solids.
4. Build back plate as a separate single solid with raised lip and ridge.
5. Keep each part as one editable solid; avoid shell+loft+sweep first pass.

### Approximation Notes
- Prefer robust boolean-driven geometry over shell on lofted surfaces in first pass.
- Add aesthetic fillets only as optional guarded final polish."#
                .to_string(),
        });
    }

    if lower.contains("snap") && lower.contains("lid") {
        return Some(FallbackPlan {
            template_id: "snap_lid_box",
            reason: "planner failed repeatedly; switched to deterministic snap-lid box plan"
                .to_string(),
            plan_text: r#"### Object Analysis
- Rectangular box with separate snap lid.

### CadQuery Approach
- Primary path: explicit outer solids + inner subtraction for box and lid.
- Optional fallback: simple friction-fit lip with conservative clearances.

### Build Plan
1. Create base box outer body and subtract inner cavity.
2. Create separate lid body with matching insertion lip.
3. Add minimal snap details using simple rectangular tabs.
4. Keep both parts as single editable solids.

### Approximation Notes
- Decorative fillets are deferred until after topology is stable."#
                .to_string(),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chooses_back_plate_template() {
        let tpl = maybe_template_for_part("back_plate", "removable back plate with lip and ridge")
            .expect("back_plate template should be selected");
        assert_eq!(tpl.template_id, "plate_with_lip_ridge");
        assert!(tpl.code.contains("result ="));
    }

    #[test]
    fn chooses_housing_template() {
        let tpl = maybe_template_for_part("housing", "main enclosure housing with slots")
            .expect("housing template should be selected");
        assert_eq!(tpl.template_id, "simple_housing_with_slots");
        assert!(tpl.code.contains("housing ="));
    }

    #[test]
    fn emits_enclosure_plan_fallback() {
        let plan = maybe_fallback_plan("whoop-style housing with separate back plate")
            .expect("fallback plan should be selected");
        assert_eq!(plan.template_id, "enclosure_with_back_plate");
        assert!(plan.plan_text.contains("### Build Plan"));
    }

    #[test]
    fn whoop_prompt_matches_housing_template() {
        let tpl = maybe_template_for_part(
            "housing",
            "Single editable housing solid. Footprint 42x28mm, wall 1.8mm. Include ledge, O-ring groove, two end slots, solid side button indicator.",
        )
        .expect("Whoop housing description should match housing template");
        // Previously this failed because "solid" contains substring "lid" — fixed with word-boundary matching
        assert_eq!(tpl.template_id, "simple_housing_with_slots");
    }

    #[test]
    fn whoop_prompt_matches_backplate_template() {
        let tpl = maybe_template_for_part(
            "back_plate",
            "Single editable back plate solid. Base 30x24mm, thickness 1.5mm. Add insertion lip and O-ring ridge.",
        )
        .expect("Whoop back_plate description should match plate template");
        assert_eq!(tpl.template_id, "plate_with_lip_ridge");
    }

    #[test]
    fn whoop_prompt_triggers_fallback_plan() {
        let whoop_request = "Create a fully parametric, editable CAD model of a wrist-worn fitness tracker housing with a snap-fit back plate";
        let plan = maybe_fallback_plan(whoop_request)
            .expect("Whoop-style request should trigger enclosure fallback plan");
        assert_eq!(plan.template_id, "enclosure_with_back_plate");
    }

    #[test]
    fn housing_with_back_plate_in_description_matches_housing_template() {
        // Real-world case: the planner generates a housing description that
        // references "back plate" as a cross-part constraint.  The template
        // selector must NOT misclassify this as a plate part.
        let tpl = maybe_template_for_part(
            "housing",
            "Main body: Extrude a rounded rectangle. Shell from bottom. Create internal ledge for back plate. Add O-ring groove. Cut band slots.",
        )
        .expect("housing with 'back plate' reference in description should still match housing template");
        assert_eq!(tpl.template_id, "simple_housing_with_slots");
    }

    #[test]
    fn body_part_name_matches_housing_template() {
        let tpl =
            maybe_template_for_part("main_body", "Primary enclosure body with internal cavity")
                .expect("main_body should match housing template");
        assert_eq!(tpl.template_id, "simple_housing_with_slots");
    }

    #[test]
    fn template_name_does_not_false_match_plate() {
        let tpl = maybe_template_for_part(
            "template_part",
            "generic helper part used for staging geometry",
        );
        assert!(
            tpl.is_none(),
            "template_part should not map to plate template"
        );
    }
}
