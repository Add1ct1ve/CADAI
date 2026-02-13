# AI Generation Capabilities — Expansion Roadmap

> This document tracks the phased expansion of the AI Build123d generation pipeline. Each sub-phase (e.g. "1.2") is a self-contained implementation task that can be tackled with "implement phase X.Y". The goal is to dramatically improve generation quality, success rate, and user experience without exceeding ~10% of the context window.

**Status Legend:**
- ⬜ Not started
- 🟡 In progress
- ✅ Complete
- ⏸️ Blocked/Waiting
- 🔴 Has bugs/needs fix

**Priority Legend:**
- P0 = Critical (highest impact on generation quality)
- P1 = Important (significant improvement)
- P2 = Nice to have (quality-of-life)
- P3 = Advanced/Future

---

## Current State

The AI generation pipeline uses ~6,500 tokens of system prompt across these components:

| Component | File | Tokens (approx) | Purpose |
|-----------|------|-----------------|---------|
| System prompt builder | `src-tauri/src/agent/prompts.rs` | — | Assembles prompt from YAML rules |
| Default rules | `agent-rules/default.yaml` | ~4,500 | Code requirements, spatial rules, cookbook, etc. |
| Geometry advisor | `src-tauri/src/agent/design.rs` | ~800 | Design plan before code generation |
| Code reviewer | `src-tauri/src/agent/review.rs` | ~600 | Post-generation correctness check |
| Error parser | `src-tauri/src/agent/validate.rs` | — | Traceback parsing + code extraction |
| Planner | `src-tauri/src/commands/parallel.rs` | ~600 | Single vs multi-part decomposition |

**Context budget:** Using Kimi K2.5 (256K context) — current usage is ~2.5%, target after all expansions is ~10.3%.

---

## Phase 1: Knowledge Base Expansion (P0)

> **Goal:** Give the AI dramatically more reference material for correct Build123d usage. Target: ~12,000 tokens added to system prompt.
>
> **Files affected:** `agent-rules/default.yaml`, `agent-rules/printing-focused.yaml`, `agent-rules/cnc-focused.yaml`

### 1.1 Expanded Cookbook — 25 New Recipes ✅

| Feature | Status | Notes |
|---------|--------|-------|
| Spur gear recipe | ✅ | Simplified involute via polyline tooth + rotate loop, 12 teeth, center bore |
| Hex bolt recipe | ✅ | Polygon hex head + shaft + chamfer (no threads — helix unreliable) |
| Compression spring recipe | ✅ | `Wire.makeHelix()` + sweep circle, 5 coils |
| Snap-fit hook recipe | ✅ | Cantilever with barb — polyline extrude |
| Snap-fit clip recipe | ✅ | U-clip with detent bumps, push-fit style |
| Electronics enclosure recipe | ✅ | Box + shell + corner standoff cylinders + screw holes |
| Raspberry Pi case recipe | ✅ | 92×63mm real dims, USB-C/HDMI/SD cutouts, standoffs |
| Pipe elbow recipe | ✅ | Sweep annular profile along 90° arc |
| Pipe T-junction recipe | ✅ | Outer/inner cylinder booleans (shell approach was unreliable) |
| Hinge (pin style) recipe | ✅ | Two plates with alternating cylinder knuckles |
| Living hinge recipe | ✅ | 0.4mm thin flexure bridge between rigid blocks |
| Knurled cylinder recipe | ✅ | Simplified circumferential V-grooves via rotate loop |
| Dovetail joint recipe | ✅ | Trapezoidal profiles — male and female pieces |
| Finger joint (box joint) recipe | ✅ | Interlocking rectangular tabs via loop cuts |
| Bearing seat recipe | ✅ | Revolved stepped bore with shoulder + snap-ring groove |
| Mounting bracket (L-shape) recipe | ✅ | L-polyline extrude + triangular gusset + hole pattern |
| Shelf bracket recipe | ✅ | Triangular plate with fillet and mounting holes |
| Pulley/wheel recipe | ✅ | Revolve with V-groove profile + center bore |
| Cam profile recipe | ✅ | Eccentric disc with shaft bore |
| Standoff/spacer recipe | ✅ | Hex base polygon + tube body + through-hole |
| Cable gland/grommet recipe | ✅ | Revolved stepped profile with flange and ridges |
| Battery holder recipe | ✅ | Box cavity for AA battery + terminal contact slots |
| D-sub connector cutout recipe | ✅ | Trapezoidal slot on panel plate — real DE-9 dims |
| USB-C port cutout recipe | ✅ | slot2D 8.94×3.26mm cutThruAll on panel plate |
| Keychain tag recipe | ✅ | Rounded rect plate + text emboss + ring hole |

**Implementation notes:**
- Add each recipe to the `cookbook` section of `default.yaml`
- Each recipe must be a complete, tested, runnable Build123d script
- Include `title`, `description`, and `code` fields
- Test every recipe through `python/runner.py` to ensure it actually produces valid geometry
- Keep individual recipes under 25 lines — concise is better than comprehensive
- Estimated token cost: ~6,000 tokens (25 recipes × ~240 tokens each)

---

### 1.2 Anti-Pattern Library

| Feature | Status | Notes |
|---------|--------|-------|
| Fillet-before-boolean anti-pattern | ⬜ | Show crash, explain "fillets LAST" rule |
| Shell-on-complex-body anti-pattern | ⬜ | Show failure after many boolean ops |
| Revolve-crossing-axis anti-pattern | ⬜ | Profile crosses rotation axis → crash |
| Boolean-non-overlapping anti-pattern | ⬜ | Silent no-op when bodies don't touch |
| Translate-wrong-signature anti-pattern | ⬜ | `translate(x, y, z)` vs `translate((x, y, z))` |
| Loft-mismatched-profiles anti-pattern | ⬜ | Different edge counts → twisted/invalid result |
| Fillet-radius-too-large anti-pattern | ⬜ | Radius > edge length → OCP crash |
| Sweep-without-wire anti-pattern | ⬜ | Edges vs Wire object confusion |
| Chain-too-many-booleans anti-pattern | ⬜ | >8 operations on one body → tolerance failure |
| Hole-on-wrong-face anti-pattern | ⬜ | Selector confusion ('>X' vs '>Z') |

**Implementation notes:**
- New YAML section: `anti_patterns` in `default.yaml`
- Schema per entry: `title`, `wrong_code`, `error_message`, `explanation`, `correct_code`
- New handler in `prompts.rs` to render anti-patterns into system prompt
- Estimated token cost: ~2,500 tokens (10 patterns × ~250 tokens each)
- Must add `anti_patterns` field to `AgentRules` struct in `rules.rs`

---

### 1.3 Build123d API Quick-Reference

| Feature | Status | Notes |
|---------|--------|-------|
| `loft()` parameter reference | ⬜ | `ruled`, `combine`, wire types, common errors |
| `sweep()` parameter reference | ⬜ | Path requirements, `isFrenet`, `transition` |
| `revolve()` parameter reference | ⬜ | Axis definition, angle, profile requirements |
| `shell()` parameter reference | ⬜ | Negative vs positive thickness, face selection |
| Selector string reference | ⬜ | `>Z`, `<X`, `\|Y`, `#Z`, `+Y`, `-X`, compound selectors |
| `Workplane` constructor reference | ⬜ | "XY"/"XZ"/"YZ", `.workplane(offset=)`, `.transformed()` |
| `.pushPoints()` vs `.rarray()` vs `.polarArray()` reference | ⬜ | When to use each pattern-placement method |
| `.tag()` and `.faces(tag=)` reference | ⬜ | Named geometry for reliable re-selection |

**Implementation notes:**
- New YAML section: `api_reference` in `default.yaml`
- Compact format: operation name, parameters, return type, common mistakes
- Should be terse — signature + 2-3 gotchas per operation, not full docs
- Estimated token cost: ~1,800 tokens
- New handler in `prompts.rs`, new field in `AgentRules`

---

### 1.4 Real-World Dimension Tables

| Feature | Status | Notes |
|---------|--------|-------|
| Metric fastener dimensions | ⬜ | M2–M12: head dia, head height, shaft dia, nut width |
| Common electronics dimensions | ⬜ | Raspberry Pi, Arduino, ESP32, USB-A/B/C, SD card |
| Bearing dimensions | ⬜ | 608, 6001, 6201, 6202 — ID, OD, width |
| Common object size reference | ⬜ | Phone, credit card, AA battery, coin, pen, mug |
| Clearance and press-fit tolerances | ⬜ | Hole/shaft fit classes: H7/h6, H7/p6, etc. |
| 3D printing clearance guidelines | ⬜ | Mating part gap, pin-in-hole, snap-fit deflection |
| Sheet metal bend radii | ⬜ | Min bend radius by thickness for aluminum, steel |

**Implementation notes:**
- New YAML section: `dimension_tables` in `default.yaml`
- Compact tabular format in YAML (lists of key-value pairs)
- The AI currently guesses dimensions — this gives it real numbers
- Estimated token cost: ~1,500 tokens
- Especially important for "make a bolt" or "make a phone case" requests

---

### 1.5 Few-Shot Design-to-Code Examples

| Feature | Status | Notes |
|---------|--------|-------|
| Example 1: "coffee mug" → design plan → code | ⬜ | Revolve-based, demonstrates organic-ish workflow |
| Example 2: "motor mount bracket" → design plan → code | ⬜ | Mechanical, demonstrates feature-based workflow |
| Example 3: "SD card holder" → design plan → code | ⬜ | Enclosure, demonstrates shell + cutout workflow |
| Example 4: "gear" → design plan → code | ⬜ | Patterned features, demonstrates polarArray workflow |
| Example 5: "phone stand" → design plan → code | ⬜ | Multi-feature, demonstrates loft + boolean workflow |

**Implementation notes:**
- New YAML section: `few_shot_examples` in `default.yaml`
- Schema: `user_request`, `design_plan` (abbreviated), `code`
- These teach the AI the full chain: interpret request → plan geometry → write code
- Keep plans to 3-5 lines, code to 15-25 lines
- Estimated token cost: ~2,200 tokens (5 examples × ~440 tokens each)
- New field in `AgentRules`, new handler in `prompts.rs`

---

## Phase 2: Pipeline Intelligence (P0)

> **Goal:** Make the retry/error handling pipeline smarter. Instead of treating all failures the same, classify errors and apply targeted fixes.
>
> **Files affected:** `src-tauri/src/agent/validate.rs`, `src-tauri/src/commands/chat.rs`, `src-tauri/src/agent/design.rs`, `src-tauri/src/agent/review.rs`, `src-tauri/src/commands/parallel.rs`

### 2.1 Error Classification Engine

| Feature | Status | Notes |
|---------|--------|-------|
| Syntax error detection | ⬜ | SyntaxError, IndentationError — code-level bug |
| Geometry kernel error detection | ⬜ | OCP.StdFail_NotDone, BRepBuilderAPI — shape-level failure |
| Topology error detection | ⬜ | Shell/fillet/boolean failures — operation-level |
| API misuse detection | ⬜ | AttributeError, TypeError on CQ methods — wrong usage |
| Import/runtime error detection | ⬜ | NameError, ModuleNotFoundError — environment issue |
| Error category enum | ⬜ | `ErrorCategory` enum with variants for each class |
| Classification function | ⬜ | `classify_error(stderr) -> ErrorCategory` |
| Structured context extraction | ⬜ | Extract failing operation name, line, parameters |

**Implementation notes:**
- Extend `validate.rs` with `ErrorCategory` enum and classifier
- Current `parse_traceback()` already extracts error type — extend it with classification
- Each category maps to a different retry strategy (see 2.2)
- `StructuredError` gets a new `category: ErrorCategory` field
- Pattern matching on error messages:
  - `StdFail_NotDone` + `fillet` → topology/fillet-too-large
  - `StdFail_NotDone` + `shell` → topology/shell-failure
  - `BRepBuilderAPI` → geometry/construction-failure
  - `SyntaxError` → syntax
  - `AttributeError` on `cq.` → api-misuse

---

### 2.2 Smart Retry Strategies

| Feature | Status | Notes |
|---------|--------|-------|
| Syntax error strategy | ⬜ | Direct fix: show error line, ask AI to fix that specific line |
| Fillet-too-large strategy | ⬜ | Reduce fillet radius by 50%, or remove fillets entirely |
| Shell failure strategy | ⬜ | Switch to manual box subtraction for hollowing |
| Boolean failure strategy | ⬜ | Ensure overlap by extending cutting tools 1mm beyond target |
| Loft/sweep failure strategy | ⬜ | Fall back to revolve or stacked extrudes |
| General geometry strategy | ⬜ | Simplify: replace curves with straight segments, reduce feature count |
| Strategy selection function | ⬜ | `get_retry_strategy(error_category) -> RetryStrategy` |
| Strategy-aware retry prompt builder | ⬜ | `build_retry_prompt(code, error, strategy) -> String` |

**Implementation notes:**
- Replace the current hardcoded 3-attempt escalation in `auto_retry` (`chat.rs`)
- Each `RetryStrategy` produces a tailored prompt instead of generic "simplify everything"
- Attempt 1: targeted fix for the specific error category
- Attempt 2: targeted fix + simplification of the failing operation class
- Attempt 3: full simplification (current behavior — primitives only)
- The prompt should include the specific anti-pattern if one matches
- New struct `RetryStrategy` with `prompt_prefix`, `simplification_level`, `forbidden_operations`

---

### 2.3 Design Plan Validation

| Feature | Status | Notes |
|---------|--------|-------|
| Dimension feasibility check | ⬜ | Reject plans with impossible dimensions (negative, >10m, <0.01mm) |
| Operation compatibility check | ⬜ | Flag risky combos (shell after many booleans, large fillet on small edges) |
| Required field check | ⬜ | Ensure plan has dimensions, operations, build sequence |
| Risk score calculation | ⬜ | 0-10 score based on operation complexity and known failure patterns |
| Plan rejection with feedback | ⬜ | If risk > 7, re-prompt the geometry advisor with specific concerns |
| Deterministic parsing of plan | ⬜ | Extract operation list, dimensions, sequence from free-text plan |

**Implementation notes:**
- New function in `design.rs`: `validate_plan(plan_text: &str) -> PlanValidation`
- `PlanValidation` struct: `is_valid`, `risk_score`, `warnings`, `rejected_reason`
- Called in `parallel.rs` between geometry planning and code generation
- If plan is rejected, re-call `plan_geometry()` with the rejection feedback appended
- Deterministic checks (regex/parsing) — no extra AI calls needed
- Rules: shell after >3 booleans = +3 risk, fillet >5mm on features <20mm = +2 risk, etc.

---

### 2.4 Review Cross-Check Against Plan

| Feature | Status | Notes |
|---------|--------|-------|
| Pass design plan to reviewer | ✅ | Include plan text in review prompt |
| Check feature completeness against plan | ✅ | Every planned feature should appear in code |
| Check dimensions match plan | ✅ | Planned 50mm should be 50mm in code, not 40mm |
| Check operation sequence matches plan | ✅ | If plan says "revolve then shell", code shouldn't "extrude then shell" |
| Enhanced reviewer prompt | ✅ | Additional checklist items for plan compliance |

**Implementation notes:**
- Modify `review_code()` in `review.rs` to accept optional `design_plan: Option<&str>`
- Extend `REVIEW_SYSTEM_PROMPT` with plan-compliance section
- Update callers in `parallel.rs` to pass the design plan through
- Add items 18-21 to the reviewer checklist for plan compliance
- The reviewer should APPROVE if code achieves the intent even if it uses different operations

---

### 2.5 Token & Cost Tracking

| Feature | Status | Notes |
|---------|--------|-------|
| Token counting per AI call | ✅ | All 4 providers (Claude, OpenAI, Gemini, Ollama) return `TokenUsage` from `complete()` and `stream()` |
| Accumulate per-generation totals | ✅ | `generate_parallel` sums across design + plan + generate + review phases |
| Cost estimation by provider | ✅ | `ai/cost.rs` rate table: Claude, OpenAI, DeepSeek, Qwen, Kimi, Gemini, Ollama |
| Surface to frontend | ✅ | `MultiPartEvent::TokenUsage` + `StreamEvent.token_usage` with cost_usd |
| Display in chat UI | ✅ | Small badge: "1,247 tokens / $0.003" — shows "free (local)" for Ollama |
| Generation history tracking | ⬜ | Store token usage in AppState for session summary (deferred) |

**Implementation notes:**
- Modify `AiProvider` trait: `complete()` and `stream()` return `TokenUsage` alongside response
- `TokenUsage` struct: `prompt_tokens`, `completion_tokens`, `total_tokens`
- New `MultiPartEvent::TokenUsage { phase, tokens }` variant
- Cost calculation in Rust (avoid frontend price hardcoding) — provider-specific rates
- Optional: disable for Ollama (local, no cost)
- Frontend: show in a collapsed "details" section under each AI message

---

## Phase 3: Prompt Engineering Advances (P1)

> **Goal:** Improve the quality of prompts and response handling without changing pipeline architecture. Target: ~3,500 tokens added.
>
> **Files affected:** `agent-rules/default.yaml`, `src-tauri/src/agent/prompts.rs`, `src-tauri/src/agent/design.rs`

### 3.1 Structured Output Enforcement

| Feature | Status | Notes |
|---------|--------|-------|
| Strict response template in prompt | ✅ | Output Structure section in `prompts.rs` with `<CODE>` tag instructions |
| Separator tokens for parsing | ✅ | `<CODE>...</CODE>` tags added to prompt and YAML presets |
| Response validation before extraction | ✅ | 3-tier cascade in `extract.rs` validates each format before accepting |
| Fallback extraction for non-compliant responses | ✅ | Heuristic tier catches bare code blocks with Build123d markers |
| Multi-format extraction | ✅ | XML tags → markdown fence → heuristic cascade in `extract.rs` |
| Extraction success rate tracking | ✅ | `log::warn!` when no code block found; `ExtractionFormat` enum tracks which tier matched |

**Implementation notes:**
- Update `response_format` section in `default.yaml` with strict template
- Update `validate.rs` `extract_python_code()` to try multiple extraction patterns
- Add structured tags to the prompt: the AI should output `<CODE>` blocks
- Fallback: if `<CODE>` not found, try ` ```python `, then try last code-like block
- Current extraction regex works well — this phase hardens it for edge cases
- Estimated token cost: ~400 tokens added to prompt

---

### 3.2 Manufacturing-Aware Design Phase

| Feature | Status | Notes |
|---------|--------|-------|
| Inject active preset into geometry advisor | ✅ | Manufacturing YAML appended to geometry advisor system prompt |
| Manufacturing constraints in design plan | ✅ | format_manufacturing_constraints() renders YAML as markdown |
| Preset-specific design guidance | ✅ | Printing/CNC/default presets each inject their constraints |
| Cross-reference plan against manufacturing rules | ✅ | Advisor prompt instructs plan MUST respect constraints |

**Implementation notes:**
- `plan_geometry()` and `plan_geometry_with_feedback()` accept optional `manufacturing_context: Option<&str>`
- `format_manufacturing_constraints()` renders YAML manufacturing data as markdown bullets
- `parallel.rs` loads AgentRules at call site, extracts manufacturing text, passes to design phase
- Estimated token cost: ~800 tokens (varies by preset)

---

### 3.3 Dimension Estimation Guidance

| Feature | Status | Notes |
|---------|--------|-------|
| "When to guess" rules | ✅ | OK to guess: common objects (mug, bolt). NOT OK: custom parts |
| Proportional reasoning rules | ✅ | "A handle is ~120mm long, a knob is ~30mm diameter" |
| Scale reference anchors | ✅ | "A human hand is ~190mm long" for ergonomic parts |
| "Ask vs guess" decision tree | ✅ | If user says "small" → 20-40mm. "Large" → 100-300mm |
| Relative sizing rules | ✅ | "The lid should be 2mm larger than the box in X and Y" |

**Implementation notes:**
- New YAML section: `dimension_guidance` with 5 sub-categories in all 3 presets
- `when_to_estimate`: decision rules for guess vs ask
- `size_classes`: tiny/small/medium/large/extra-large with mm ranges
- `scale_anchors`: human hand, finger, battery, credit card references
- `proportional_reasoning`: typical dimensions for common objects (mug, knob, hook, etc.)
- `relative_sizing`: clearance, wall thickness, press-fit, nesting rules
- Old "Never assume dimensions" rule replaced with softer guidance
- Rendered into both code generation prompt (`prompts.rs`) and geometry advisor (`design.rs`)
- `parallel.rs` passes combined manufacturing + dimension guidance to geometry advisor
- Estimated token cost: ~800 tokens

---

### 3.4 Failure Case Prompting

| Feature | Status | Notes |
|---------|--------|-------|
| Self-diagnosis scenarios | ✅ | 7 rules: fillet, shell, loft, revolve, boolean, sweep, chamfer |
| Pre-emptive warning triggers | ✅ | 6 rules: shell-after-booleans, fillet-before-boolean, loft profiles, etc. |
| Alternative operation suggestions | ✅ | 6 rules: loft→extrude, shell→hollow, sweep→segments, etc. |
| Complexity self-assessment | ✅ | 5 rules: line count, operation count, build order, tag usage |
| Error recovery checklist | ✅ | 8-item pre-output checklist for the AI to self-check before outputting code |

**Implementation notes:**
- New YAML section: `failure_prevention` in all 3 presets (default, printing, cnc)
- 5 categories, 32 rules total as "if X then Y" rules embedded in the system prompt
- Rendered in both code generation prompt (`prompts.rs`) and geometry advisor (`design.rs`)
- `parallel.rs` passes failure prevention context to geometry advisor
- Complements anti-patterns (1.2) — those are code examples, these are proactive decision rules
- Estimated token cost: ~1,500 tokens

---

## Phase 4: Pipeline Architecture Improvements (P1-P2)

> **Goal:** Major code changes to the generation pipeline for higher success rates and better user experience.
>
> **Files affected:** `src-tauri/src/commands/parallel.rs`, `src-tauri/src/commands/chat.rs`, `src-tauri/src/agent/` (new modules), `python/runner.py`, `src/lib/components/Chat.svelte`

### 4.1 Execution-Based Validation Loop

| Feature | Status | Notes |
|---------|--------|-------|
| Execute code before returning to user | ✅ | Run through `runner.py` after generation |
| Capture execution errors internally | ✅ | Parse stderr, classify error (uses 2.1) |
| Auto-fix loop (max 3 internal attempts) | ✅ | Re-prompt AI with error, get fix, re-execute |
| Success = show result + code to user | ✅ | Only surface the working version |
| All-attempts-failed = show last code + error | ✅ | User sees the best attempt + clear error |
| Progress events for internal retries | ✅ | "Validating code... fixing issue... retrying..." |
| Timeout per execution attempt | ✅ | Kill runaway scripts after 30s |

**Implementation notes:**
- Backend executes code after generation via `executor::validate_and_retry()`
- New module: `src-tauri/src/agent/executor.rs` — core validation loop
- Flow: generate → execute → if error: classify → retry with targeted prompt (non-streaming) → execute → ...
- Reuses `runner.py` subprocess execution, `parse_traceback`, `get_retry_strategy`, `build_retry_prompt`
- `MultiPartEvent` has new variants: `ValidationAttempt`, `ValidationSuccess`, `ValidationFailed`
- `FinalCode` now carries optional `stl_base64`, `Done` carries `validated` flag
- Frontend skips execution when `validated=true` and renders STL directly
- Graceful fallback: if Python venv not set up, existing frontend-driven flow works unchanged
- `auto_retry` command kept as manual fallback (not removed)

---

### 4.2 Iterative Refinement Mode

| Feature | Status | Notes |
|---------|--------|-------|
| Step-by-step build mode | ✅ | Generate base shape → verify → add feature → verify → ... |
| Step plan from design phase | ✅ | Parse design plan steps into a build sequence |
| Per-step execution check | ✅ | After each step, execute to verify it works |
| Step rollback on failure | ✅ | If step N fails after 3 retries, skip it and continue from step N-1 |
| Incremental code accumulation | ✅ | Each step appends to previous working code |
| Step-by-step progress UI | ✅ | Show each step completing in the frontend with viewport updates |

**Implementation notes:**
- Module: `src-tauri/src/agent/iterative.rs`
- Auto-triggered by complexity: 4+ build steps OR any risky operation (shell, loft, sweep, revolve)
- Simple requests continue using the existing single-shot path
- Parse design plan `### Build Plan` section into numbered `BuildStep` entries
- For each step: AI generates code → execute → on failure retry up to 3 times → skip if still failing
- Viewport updates after each successful step so user sees the model being built incrementally
- After completion, "Retry Skipped Steps" button appears if any steps were skipped
- Retry command (`retry_skipped_steps`) re-runs the iterative loop for only skipped steps, starting from current code
- Uses `provider.complete()` (non-streaming) per step; frequent StepStarted/StepComplete events provide progress

---

### 4.3 Code Diff and Modification Support

| Feature | Status | Notes |
|---------|--------|-------|
| Detect modification requests | ✅ | Regex-based intent detection in `agent/modify.rs` — 13 patterns |
| Send existing code as context | ✅ | Frontend sends editor code via `existing_code` parameter |
| Modification-specific prompt | ✅ | MODIFICATION_INSTRUCTIONS addendum skips design/planning phases |
| Code diff display | ✅ | Line-level diff via `similar` crate, rendered in Chat.svelte |
| Preserve variable names and structure | ✅ | Modification prompt instructs AI to keep existing structure |
| Selective regeneration | ✅ | Modification branch skips design plan + planner, streams directly |

**Implementation notes:**
- Currently every request generates code from scratch, even "make it 5mm taller"
- New command: `modify_code` in `chat.rs` that takes `existing_code + modification_request`
- New system prompt mode: "modification mode" that tells the AI to edit, not rewrite
- Frontend: if there's already code in the editor, send it with the request
- Diff display: use a simple line-by-line diff algorithm in TypeScript
- The modification prompt should say: "Here is the current Build123d code. The user wants to [change]. Modify the code to implement this change. Return the complete modified code."

---

### 4.4 Multi-Model Consensus

| Feature | Status | Notes |
|---------|--------|-------|
| Send request to 2-3 providers in parallel | ✅ | Uses same provider at 2 temperatures (0.3 + 0.8) for diversity |
| Execute all results | ✅ | Test each code output through runner sequentially |
| Pick best result | ✅ | Scoring: ops*10 + lines + 1000 execution bonus; highest wins |
| Fallback to single model on timeout | ✅ | Falls through to normal single-shot if consensus produces no code |
| Cost-aware model selection | ✅ | Same provider, different temperatures — no extra API key needed |
| User opt-in toggle | ✅ | Settings checkbox: "Enable consensus mode" (default off) |

**Implementation notes:**
- New module: `src-tauri/src/agent/consensus.rs`
- Spawn 2-3 parallel generation tasks with different providers
- Execute each result through `runner.py` — first success wins
- If multiple succeed, pick the one with more features / fewer lines
- Default off (costs 2-3x tokens). Enabled via settings toggle.
- Requires user to have API keys for multiple providers configured
- Can also use same provider with different temperature settings

---

## Phase 5: Frontend UX for AI Quality (P2)

> **Goal:** Give users visibility and control over the AI generation process.
>
> **Files affected:** `src/lib/components/Chat.svelte`, `src/lib/components/` (new components), `src/lib/types/index.ts`, `src/lib/stores/`

### 5.1 Interactive Design Plan Editor ✅

| Feature | Status | Notes |
|---------|--------|-------|
| Show design plan in an editable panel | ✅ | Before code gen starts, show the geometry plan |
| Edit plan text inline | ✅ | User can modify dimensions, features, approach |
| Approve/reject plan | ✅ | "Generate Code" button only after user approves |
| Auto-approve option | ✅ | Setting to skip manual approval for speed |
| Plan diff on re-generation | ✅ | If user modifies request, show what changed in plan |
| Plan templates | ✅ | Quick-start templates for common object types |

**Implementation notes:**
- New component: `DesignPlanEditor.svelte`
- Currently the plan is shown as read-only text in the chat stream
- This phase makes it an interactive step: plan appears → user can edit → clicks "Generate"
- Frontend sends the (possibly edited) plan text to the code generation phase
- Requires new Tauri command: `generate_from_plan(plan_text, user_request)`
- The `MultiPartEvent::DesignPlan` event now triggers the editor instead of just displaying text

---

### 5.2 Generation Confidence Indicator ✅

| Feature | Status | Notes |
|---------|--------|-------|
| Predict success likelihood | ✅ | Based on plan risk score + cookbook familiarity |
| Green/yellow/red badge | ✅ | Compact badge during streaming, expanded after generation |
| Complexity scoring algorithm | ✅ | base = 100-(risk*10), cookbook bonus ±15, clamped 0-100 |
| Known-pattern detection | ✅ | `match_cookbook()` with operation overlap + title word boost |
| Warning messages for yellow/red | ✅ | Loft+shell, novel combo, high complexity warnings |
| Post-generation confidence update | ✅ | Frontend adjusts score on ReviewComplete/ValidationSuccess/ValidationFailed |

**Implementation notes:**
- New utility module: `src-tauri/src/agent/confidence.rs`
- Analyze the design plan text (or user request) for complexity signals
- Scoring: each operation type has a risk weight. `box` = 0, `loft` = 3, `shell` = 2, `sweep` = 3
- Total risk score maps to confidence: 0-3 = green, 4-7 = yellow, 8+ = red
- Frontend: small colored badge next to "Generating..." message
- After generation: update based on whether code review found issues

---

### 5.3 Enhanced Multi-Part Progress ✅

| Feature | Status | Notes |
|---------|--------|-------|
| Per-part progress cards | ✅ | Card-based UI with status icons, descriptions, constraints |
| Part preview rendering | ✅ | Click "Preview" to show individual part STL in main viewport |
| Retry button per part | ✅ | "Retry" button on failed parts, calls `retry_part` command |
| Part code viewer | ✅ | Expandable inline code viewer per part card |
| Assembly progress visualization | ✅ | Parts appear in viewport via `PartStlReady` as they complete |
| Part dependency display | ✅ | Constraints shown as bulleted list under each card |

**Implementation notes:**
- New component: `src/lib/components/MultiPartProgress.svelte`
- New Rust functions: `execute_cad_isolated()` (runner.rs), `execute_with_timeout_isolated()` (executor.rs)
- New event variants: `PartCodeExtracted`, `PartStlReady` on `MultiPartEvent`
- New command: `retry_part` — re-generates a single failed part with streaming
- Background per-part STL tasks spawned after code extraction, non-blocking
- Cards persist after generation for user interaction (preview, code, retry)
- Assembly STL cached for "Show Full Assembly" button

---

### 5.4 Generation History & Comparison ✅

| Feature | Status | Notes |
|---------|--------|-------|
| Store generation attempts | ✅ | In-memory store, 20 entries max, auto-evicts oldest non-pinned |
| Side-by-side code diff | ✅ | LCS line-by-line diff with green/red highlighting |
| Click-to-preview 3D comparison | ✅ | Preview A/B buttons swap STL in main viewport |
| Pin/favorite a generation | ✅ | Pinned entries survive eviction, confirm before delete |
| Roll back to previous attempt | ✅ | Restore code + STL from any entry via custom event |
| Generation metadata | ✅ | Tokens (in/out/total), cost, model, provider, duration, confidence, type, retries |

**Implementation notes:**
- New store: `generationHistory.svelte.ts` — `$state` + `getGenerationHistoryStore()` factory
- New component: `GenerationHistory.svelte` — list/detail/compare views in RightPanel History tab
- New utility: `utils/diff.ts` — LCS diff producing `DiffLine[]` (reuses existing type)
- Click-to-preview (not split viewport) — avoids duplicating WebGL setup
- Custom event `generation-history:restore` for cross-component restore
- In-memory only (STL base64 ~100KB-1MB each); resets on app restart

---

## Phase 6: Advanced Knowledge and Reasoning (P3)

> **Goal:** Push the AI toward expert-level Build123d usage with deep domain knowledge.
>
> **Files affected:** `agent-rules/default.yaml`, `src-tauri/src/agent/` (new modules), `src-tauri/src/agent/prompts.rs`

### 6.1 Parametric Design Pattern Library

| Feature | Status | Notes |
|---------|--------|-------|
| Enclosure pattern | ✅ | Template: base + lid + screw bosses + lip + gasket groove |
| Shaft/axle pattern | ✅ | Template: cylindrical with keyways, shoulders, snap ring grooves |
| Rotational body pattern | ✅ | Template: revolve profile with standard features (threads, grooves, flanges) |
| Plate/bracket pattern | ✅ | Template: flat plate with holes, stiffening ribs, mounting features |
| Tube/pipe pattern | ✅ | Template: hollow cylinder with fittings, bends, flanges |
| Spring pattern | ✅ | Template: compression, tension, torsion — helix parameters |
| Gear pattern | ✅ | Template: spur, bevel — module, tooth count, pressure angle |
| Pattern selection logic | ✅ | Keyword + operation overlap matching in confidence scoring |

**Implementation notes:**
- New YAML section: `design_patterns` in `default.yaml`
- Each pattern is a higher-level template than a cookbook recipe
- Pattern includes: description, parameter list, base code template, common variants
- The AI uses these as starting points and customizes parameters
- Pattern matching: keyword detection in design plan ("enclosure" → enclosure pattern)
- More structured than cookbook recipes — these are parameterized templates
- Estimated token cost: ~2,000 tokens

---

### 6.2 Cross-Operation Reasoning Rules

| Feature | Status | Notes |
|---------|--------|-------|
| Fillet-after-boolean interaction rules | ✅ | "New edges from boolean are often short → use smaller fillet" |
| Shell-after-fillet interaction rules | ✅ | "Shell on filleted body can fail → shell first, fillet after" |
| Loft-then-shell interaction rules | ✅ | "Lofted bodies often have thin regions → check min thickness" |
| Boolean-chain-limit rules | ✅ | "After 5+ booleans, tolerances accumulate → merge intermediate results" |
| Extrude-on-face interaction rules | ✅ | "Face selectors may become ambiguous after booleans → use tags" |
| Sweep-with-boolean interaction rules | ✅ | "Swept bodies have complex topology → avoid shell/fillet on them" |
| Revolve-then-cut interaction rules | ✅ | "Cuts on revolved bodies need careful face selection" |
| Operation ordering meta-rules | ✅ | "General order: base shape → features → booleans → fillets → shell" |

**Implementation notes:**
- New YAML section: `operation_interactions` in `default.yaml`
- Tribal knowledge about how Build123d operations interact with each other
- Format: "If operation A followed by operation B, then [rule/warning/alternative]"
- These are currently learned the hard way through failures — codify them
- The AI reads these before generating and plans operation order accordingly
- Complements anti-patterns (1.2) — those show single-operation failures, these show interaction failures
- Estimated token cost: ~1,200 tokens

---

### 6.3 Context-Aware Session Memory

| Feature | Status | Notes |
|---------|--------|-------|
| Track failures within conversation | ✅ | SessionMemory records operations, success/failure, error category per attempt |
| Avoid repeated failure patterns | ✅ | Session learnings injected into system prompt with "Do NOT repeat failed approaches" |
| Accumulate working patterns | ✅ | Successful operation combos tracked as "reliable combinations" |
| Session context injection | ✅ | build_context_section() appended to system prompt in generate_parallel/generate_from_plan |
| Cross-generation learning | ✅ | PipelineOutcome captures success/error, recorded in session memory after each generation |
| Session summary on completion | ✅ | Session Context section lists all attempts with numbered outcomes + learnings |

**Implementation notes:**
- New module: `src-tauri/src/agent/memory.rs`
- `SessionMemory` struct stored in `AppState` — per-conversation
- After each generation attempt: record success/failure, operations used, error type
- Before each new generation: inject memory summary into system prompt
- Memory format: "Previous attempts in this session: [1] loft+shell → shell failed, [2] loft+manual hollow → success"
- Clear on new conversation / "New Chat"
- No persistent storage — session-only (session ends when user starts new chat)

---

### 6.4 Build123d Version-Aware Prompts

| Feature | Status | Notes |
|---------|--------|-------|
| Detect installed Build123d version | ✅ | `detect_build123d_version()` in installer.rs |
| Version-specific API availability | ✅ | Feature detection with warnings in prompt |
| Disable unavailable operations in prompt | ✅ | Version notes section warns about unavailable features |
| Version-specific cookbook filtering | ✅ | Recipes with `min_version` filtered at prompt build time |
| Version check on app startup | ✅ | Cached in `AppState.build123d_version`, shown in StatusBar |
| Upgrade recommendation | ✅ | Prompt includes "Consider upgrading" for older versions |

**Implementation notes:**
- New function in `commands/python.rs`: `detect_build123d_version() -> String`
- Call on app startup, store in `AppState`
- Pass version string to `build_system_prompt()` — prompts.rs adds version-aware notes
- Cookbook recipes get optional `min_version` field — filtered at prompt build time
- Estimated token cost: ~200 tokens (just version notes, no new content)

---

## Token Budget Summary

| Phase | Added Tokens | Cumulative | % of 256K |
|-------|-------------|------------|-----------|
| Current | 6,500 | 6,500 | 2.5% |
| 1.1 Cookbook expansion | 6,000 | 12,500 | 4.9% |
| 1.2 Anti-patterns | 2,500 | 15,000 | 5.9% |
| 1.3 API reference | 1,800 | 16,800 | 6.6% |
| 1.4 Dimension tables | 1,500 | 18,300 | 7.1% |
| 1.5 Few-shot examples | 2,200 | 20,500 | 8.0% |
| 3.1 Structured output | 400 | 20,900 | 8.2% |
| 3.2 Manufacturing-aware design | 800 | 21,700 | 8.5% |
| 3.3 Dimension guidance | 800 | 22,500 | 8.8% |
| 3.4 Failure case prompting | 1,500 | 24,000 | 9.4% |
| 6.1 Design patterns | 2,000 | 26,000 | 10.2% |
| 6.2 Operation interactions | 1,200 | 27,200 | 10.6% |
| **Total after all knowledge phases** | **~20,700** | **~27,200** | **~10.6%** |

> Phases 2, 4, and 5 are code-only changes — they don't add to the system prompt token count.
> Remaining context for conversation after all expansions: ~228,800 tokens (89.4%).

---

## Priority / Effort Matrix

| Phase | Priority | Effort | Impact | Notes |
|-------|----------|--------|--------|-------|
| 1.1 Expanded Cookbook | P0 | Medium | High | More reference = better generation accuracy |
| 1.2 Anti-Patterns | P0 | Medium | High | Prevents the most common failures |
| 1.3 API Quick-Reference | P0 | Low | Medium | Compact but valuable for tricky operations |
| 1.4 Dimension Tables | P1 | Low | Medium | Eliminates guessing for standard parts |
| 1.5 Few-Shot Examples | P1 | Medium | High | Teaches full workflow by example |
| 2.1 Error Classification | P0 | Medium | High | Foundation for smart retries |
| 2.2 Smart Retry Strategies | P0 | Medium | Very High | Targeted fixes >> generic simplification |
| 2.3 Design Plan Validation | P1 | Low | Medium | Catches bad plans before wasting tokens |
| 2.4 Review Cross-Check | P1 | Low | Medium | Reviewer catches plan-code mismatches |
| 2.5 Token Tracking | P2 | Low | Low | Nice for cost awareness |
| 3.1 Structured Output | P1 | Low | Medium | Reduces extraction failures |
| 3.2 Manufacturing-Aware Design | P1 | Low | Medium | Better parts for specific use cases |
| 3.3 Dimension Guidance | P1 | Low | Medium | Less "I don't know the dimensions" responses |
| 3.4 Failure Case Prompting | P1 | Low | High | Proactive failure avoidance |
| 4.1 Execution Validation | ✅ | High | Very High | **Biggest single UX improvement** |
| 4.2 Iterative Refinement | ✅ | High | High | Complex objects succeed more often |
| 4.3 Code Modification | ✅ | Medium | High | "Make it taller" is the #1 follow-up request |
| 4.4 Multi-Model Consensus | ✅ | Medium | Medium | Same-provider dual-temperature consensus |
| 5.1 Plan Editor | ✅ | Medium | Medium | User control over geometry planning |
| 5.2 Confidence Indicator | ✅ | Low | Low | Manages expectations |
| 5.3 Multi-Part Progress | ✅ | Medium | Medium | Better visibility for multi-part |
| 5.4 Generation History | ✅ | Medium | Medium | Compare and roll back attempts |
| 6.1 Design Patterns | P2 | Medium | Medium | Higher-level templates |
| 6.2 Operation Interactions | P1 | Low | High | Tribal knowledge codified |
| 6.3 Session Memory | P2 | Medium | Medium | Learns from failures within session |
| 6.4 Version-Aware Prompts | ✅ | Low | Low | Detects Build123d version, filters prompt/cookbook |

---

## Recommended Implementation Order

For maximum impact with minimum effort:

1. **Phase 1.1 + 1.2** — Cookbook + Anti-patterns (knowledge foundation)
2. **Phase 2.1 + 2.2** — Error classification + Smart retries (pipeline intelligence)
3. **Phase 4.1** — Execution validation loop (biggest UX improvement)
4. **Phase 1.3 + 1.4** — API reference + Dimension tables
5. **Phase 3.4 + 6.2** — Failure prevention + Operation interactions
6. **Phase 1.5 + 3.3** — Few-shot examples + Dimension guidance
7. **Phase 4.3** — Code modification support
8. **Phase 2.3 + 2.4** — Plan validation + Review cross-check
9. **Phases 3.1 + 3.2** — Structured output + Manufacturing awareness
10. **Phase 4.2** — Iterative refinement mode
11. **Phases 5.x** — Frontend UX improvements
12. **Phases 6.x** — Advanced knowledge and reasoning

---

## How to Update This File

When completing a sub-phase:
1. Change status from ⬜ to ✅
2. Add any relevant notes (actual token count, files modified, lessons learned)
3. If bugs found, mark 🔴 and describe the issue
4. Update token budget table if actual counts differ from estimates
