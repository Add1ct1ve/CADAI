# Whoop-Style Fitness Tracker Housing - CAD Prompt

Create a fully parametric, editable CAD model of a wrist-worn fitness tracker housing with a snap-fit back plate, inspired by Whoop band design.

## General

- Units: millimeters
- Define named parameters:
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
- Create two separate solids/bodies: Housing and BackPlate.
- Origin at center of bottom face.

## Requirements

- Curved top: 7.5 mm center, 5 mm ends.
- Internal cavity with wall and top thickness control.
- Back plate ledge + o-ring groove.
- Band slots on both short ends.
- Solid button area on +Y (optional shallow indicator only).
- Back plate with insertion lip and o-ring ridge.
- Parametric and editable.
- Watertight final geometry.

## Validation Checklist

- Band slots fully cut through end faces.
- Button area is solid (no through cut).
- Back plate fits housing with tolerance.
- O-ring groove avoids band slot areas.
- Dimensions are parametric.
- Model is watertight and smooth.
