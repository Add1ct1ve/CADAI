# CadAI

CadAI is a desktop CAD application that turns natural‑language descriptions into parametric 3D models. It plans multi‑part assemblies, generates Build123d (Python) code per part, executes locally, and validates geometry.

Status: on ice. The repo is public for reference and community experimentation.

**Key Capabilities**
- Natural‑language to Build123d code.
- Multi‑part planning and parallel part generation.
- Retrieval‑augmented guidance (patterns, anti‑patterns, API references).
- Geometry validation and repair‑oriented retries.
- Local execution for fast iteration.

**How It Works**
1. You describe a design in natural language.
2. A geometry advisor drafts a plan.
3. A planner emits JSON parts with dimensions and placements.
4. Each part is generated in parallel by a code agent.
5. Python executes the code, validates geometry, and renders results.

**Tech Stack**
- Rust + Tauri backend.
- SvelteKit frontend.
- Python runner using Build123d.

**Requirements**
- Node.js and pnpm (or npm).
- Rust toolchain.
- Python 3.10+ with Build123d.

**Quick Start**
1. Install JS dependencies: `pnpm install`
2. Install Python dependencies: `python -m pip install -r python/requirements.txt`
3. Run the app: `pnpm tauri dev`

**Tests**
- Rust: `cd src-tauri && cargo test -p cadai-studio`
- Frontend checks: `pnpm check`
- E2E smoke: `pnpm test:e2e:smoke`

**Configuration Notes**
- Set your AI provider and API key in the app Settings.
- Build123d is required for local code execution and validation.
- By convention, parts are modeled at the origin and positioned during assembly using planner `position` and `instances` fields.

**Repository Layout**
- `src-tauri` Rust/Tauri backend.
- `src` Svelte UI.
- `python` Build123d runner and validation.
- `agent-rules` Prompt rulebook and patterns.
- `mechanisms` Mechanism catalog packs.

**Security Note**
CadAI executes generated Python code locally. Review any code you run and consider using a dedicated environment.

**License**
MIT (see `LICENSE` if present).
