# Capability Gaps

Broad areas where the model needs improvement. Training data should build these general skills — not fix individual test case failures.

---

## 1. Build123d API Semantics

The model knows build123d syntax but doesn't deeply understand how the API works — context managers, pending geometry, when operations actually apply.

**Symptoms observed:**
- `offset()` called in a new `BuildSketch` with no pending geometry (produces empty sketch, hollowing silently fails)
- `BuildPart(existing_part)` — passing a Part where a Plane/Location is expected
- Hallucinated classes/functions that don't exist (`HandleVector`, `ArcArc`)
- CadQuery-isms leaking through (`show_object()`, `cq.Workplane()`)

**What training data needs:**
- Diverse examples of correct context manager nesting (`BuildPart` → `BuildSketch` → `BuildLine`)
- Explicit coverage of what pending geometry is available at each scope level
- Hollowing patterns: `offset_3d()` for shell ops, explicit inner-sketch subtraction as alternative
- `BuildPart()` only accepts `Plane`/`Location` workplanes
- Negative examples showing common misuses and what goes wrong

---

## 2. Spatial & Geometric Reasoning

The model can place individual features but struggles with reasoning about how features relate to the overall part — positioning, clearances, overlap, layout.

**Symptoms observed:**
- 6 mounting holes placed in a 1×6 row instead of a 2×3 grid
- Hole patterns not centered on plate, last hole clipping the edge
- Boolean cuts splitting bodies (insufficient overlap with existing geometry)
- New geometry positioned without volumetric intersection to the existing body

**What training data needs:**
- Feature placement relative to part boundaries (inset from edges, centered on faces)
- Grid/array layout reasoning: match pattern shape to part shape (rectangular plate → rectangular grid)
- Boolean overlap awareness: additive geometry needs physical intersection, cuts need clearance from walls
- Monolithic construction over step-by-step additive assembly (less error-prone)
- Annotated examples showing the spatial reasoning behind placement choices

---

## 3. Mechanical / Engineering Domain Knowledge

The model treats prompts as pure geometry tasks without understanding the functional purpose of the part. It doesn't know what a keycap needs to do, or how a mounting plate is used.

**Symptoms observed:**
- Keycap generated as a solid block (needs to be hollow with a stem socket)
- Hex nut bore was hexagonal (must be cylindrical — the hex is only the outer profile)
- No thread generation capability (needs helix + profile sweep)
- Cable organizer slots built as additive thin walls instead of subtracted channels
- Mounting holes in a single row instead of a practical grid pattern

**What training data needs:**
- Parts annotated with functional intent ("this is hollow because it fits over a switch stem")
- Common mechanical part archetypes: enclosures, brackets, fasteners, connectors, plates
- Standard conventions: round bores, grid-pattern mounting holes, wall thickness ratios
- Understanding that "keycap" implies hollow, "mounting plate" implies distributed holes, "slot" implies subtracted channel
- Parametric mechanical features: threads (helix + swept profile), knurling, snap fits

---

## 4. Code Generation Quality

Basic Python generation issues that shouldn't happen at all.

**Symptoms observed:**
- `def` with no body followed by `except` (IndentationError)
- Same broken pattern repeated across all 4 retry attempts (model can't self-correct from error feedback)

**What training data needs:**
- This is more about base model capability than fine-tuning data
- Ensure training examples are syntactically perfect (no broken indentation in training set)
- Consider: a `compile()` pre-check in CADAI's static validator could catch these before execution, saving retry cycles
