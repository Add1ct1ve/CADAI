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
SLOT_W = {slot_w:.3}
SLOT_H = {slot_h:.3}
SLOT_D = {slot_d:.3}

outer = cq.Workplane("XY").box(L, W, H, centered=(True, True, False))
inner = cq.Workplane("XY", origin=(0, 0, WALL)).box(
    max(L - 2 * WALL, 1.0),
    max(W - 2 * WALL, 1.0),
    max(H - WALL, 1.0),
    centered=(True, True, False),
)
housing = outer.cut(inner)

slot = cq.Workplane("XY", origin=(L / 2 - SLOT_D / 2, 0, H * 0.5)).box(
    SLOT_D, SLOT_W, SLOT_H, centered=(True, True, True)
)
housing = housing.cut(slot).cut(slot.mirror("YZ"))

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
    let lower_name = part_name.to_lowercase();
    let lower_desc = description.to_lowercase();
    let combined = format!("{} {}", lower_name, lower_desc);

    if combined.contains("back_plate")
        || combined.contains("back plate")
        || combined.contains("backplate")
        || combined.contains("cover")
        || combined.contains("lid")
    {
        return Some(plate_with_lip_ridge_template(part_name, description));
    }
    if combined.contains("snap") && combined.contains("box") {
        return Some(snap_lid_box_template(part_name, description));
    }
    if combined.contains("housing") || combined.contains("enclosure") || combined.contains("case") {
        return Some(simple_housing_with_slots_template(part_name, description));
    }
    None
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
}
