# CAD AI Studio

An open-source, model-agnostic AI-powered CAD application. Describe what you want to build in natural language, and AI generates [Build123d](https://github.com/gumyr/build123d) Python code that renders interactive 3D models in real-time.

Built with Tauri 2, Svelte 5, Three.js, and a Rust backend.

---

## Fine-Tuned Models

We maintain two open-weight models fine-tuned specifically for Build123d code generation. Build123d is a niche Python CAD library with limited representation in general-purpose LLM training data, so even the best frontier models struggle with it. Our fine-tuned 32B models significantly outperform models 10x their size on this task.

### v2 — Caiden2 Build123d 32B (Recommended)

**[Add1ct1ve/caiden2-build123d-32b](https://huggingface.co/Add1ct1ve/caiden2-build123d-32b)**

- **Base model:** Qwen2 32B
- **Parameters:** 33B (BF16)
- **Training data:** 347K Build123d examples (user/assistant pairs)
- **First-pass success rate:** ~60%
- **Success with retries (up to 4 attempts):** ~88%

### v1 — Aiden Build123d 32B

**[Add1ct1ve/aiden-build123d-32b](https://huggingface.co/Add1ct1ve/aiden-build123d-32b)**

- **Base model:** Qwen2 32B
- **Parameters:** 33B (BF16/F16)
- **First-pass success rate:** ~48%
- **Success with retries (up to 3 attempts):** ~65%

### Performance vs. Frontier Models

| Model | First-pass | With retries |
|-------|-----------|--------------|
| **Caiden2 Build123d 32B (v2)** | **~60%** | **~88%** |
| Aiden Build123d 32B (v1) | ~48% | ~65% |
| Claude Opus 4.5 | ~40% | — |

Build123d is a small, specialized library. General-purpose models often hallucinate non-existent APIs, misuse context managers, and produce code with broken geometry. Our fine-tuned models are trained specifically on correct Build123d patterns, so they know the actual API surface and produce valid CAD code at much higher rates.

v2 improved on v1 primarily through better training data curation targeting Build123d API semantics, spatial reasoning, and mechanical domain knowledge.

---

## Features

- **AI Chat** — streaming responses from any provider with automatic code extraction
- **Multi-provider support** — Claude, OpenAI, Gemini, DeepSeek, Qwen, Kimi, Ollama (local), RunPod (self-hosted)
- **Interactive 3D viewport** — orbit, pan, zoom, standard views, orthographic toggle, box selection
- **Monaco code editor** — Python syntax highlighting, inline editing
- **Auto-retry** — failed code is sent back to the AI with structured error context for automatic fixing
- **Design planning** — AI generates a geometry plan before coding, reviewable in the UI
- **Multi-part generation** — complex assemblies are broken into parallel generation tasks
- **Agent rules** — YAML presets (default, 3D printing, CNC) that guide the AI with spatial reasoning rules, API patterns, and manufacturing constraints
- **Sketch tools** — lines, rectangles, circles, arcs, splines, bezier curves with constraints (tangent, fix, midpoint, symmetric, collinear)
- **3D operations** — extrude, fillet, chamfer, revolve, sweep, loft, booleans
- **Feature tree** — operation history with rollback slider and visibility toggles
- **Import/Export** — STL, STEP import and export
- **2D drawings** — orthographic projection generation (PDF, DXF export)
- **Manufacturing** — mesh validation, print orientation optimization, sheet metal unfold
- **Mechanism catalog** — browsable pattern library for common mechanical components
- **Consensus mode** — runs multiple generation attempts and picks the best result
- **Confidence scoring** — rates generation quality before execution
- **Local telemetry** — traces stored locally as JSONL for debugging (nothing sent externally)

---

## Prerequisites

- **Rust** — latest stable toolchain ([rustup.rs](https://rustup.rs))
- **Node.js** 20+ with **pnpm** (`npm install -g pnpm`)
- **Python** 3.8+ (auto-detected on first run; the app creates its own venv)
- **Git**

Platform-specific Tauri dependencies: see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

---

## Getting Started

```bash
# Clone
git clone https://github.com/Add1ct1ve/CADAI.git
cd CADAI

# Install frontend dependencies
pnpm install

# Run in dev mode (hot-reload frontend + Rust backend)
pnpm tauri dev
```

On first launch, the app will:
1. Detect your Python installation
2. Create a virtual environment in your OS app data directory
3. Install Build123d via pip

You can also set up Python manually from **Settings > Python Environment > Setup Python Environment**.

### Build for Production

```bash
pnpm tauri build
```

The built binary will be in `src-tauri/target/release/`.

---

## Configuration

Open **Settings** (gear icon) to configure:

### AI Provider

Select your provider and enter your API key. Supported providers:

| Provider | Notes |
|----------|-------|
| **Claude** | Anthropic API key. Default provider. |
| **OpenAI** | Supports custom base URL for compatible APIs. |
| **Gemini** | Google Generative AI API key. |
| **DeepSeek** | DeepSeek API key. |
| **Qwen** | Aliyun DashScope API key. |
| **Kimi** | Moonshot API key. |
| **Ollama** | Local inference, no API key needed. Set base URL (default `localhost:11434`). Enter any model name. |
| **RunPod** | Self-hosted models. Enter your RunPod API key and serverless endpoint URL. |

### Running Our Fine-Tuned Model

To use our recommended **Caiden2 Build123d 32B** model:

1. Deploy [Add1ct1ve/caiden2-build123d-32b](https://huggingface.co/Add1ct1ve/caiden2-build123d-32b) to a RunPod serverless endpoint (or any OpenAI-compatible hosting)
2. In Settings, select **RunPod (Caiden2)** as the provider
3. Enter your **RunPod API key**
4. Enter your **endpoint URL** (e.g., `https://api.runpod.ai/v2/YOUR_ENDPOINT_ID/openai/v1`)

The app automatically uses a minimal prompt for the fine-tuned model (it was trained on raw user/assistant pairs and doesn't need the full agent rules system prompt).

You can also run it locally via **Ollama** if you have enough VRAM (~20GB+ for quantized versions).

### Agent Rules

Three presets control the system prompt sent to the AI:

- **Default** — universal spatial reasoning rules, Build123d API patterns, coordinate system guidance
- **3D Printing** — adds wall thickness, overhang, bridging, and support generation constraints
- **CNC** — adds tool path, minimum feature size, and surface finish rules

---

## Project Structure

```
cadai/
├── src/                        # Svelte 5 frontend
│   ├── lib/
│   │   ├── components/         # UI (Chat, Viewport, CodeEditor, Settings, ...)
│   │   ├── stores/             # Svelte 5 rune-based state (.svelte.ts)
│   │   ├── services/           # Business logic (viewport engine, Tauri IPC, ...)
│   │   └── types/              # TypeScript interfaces
│   └── routes/                 # SvelteKit pages
├── src-tauri/                  # Rust backend
│   └── src/
│       ├── ai/                 # Provider implementations (Claude, OpenAI, Gemini, Ollama)
│       ├── agent/              # Prompt building, validation, retry logic, code extraction
│       ├── commands/           # Tauri IPC handlers (chat, CAD execution, settings, ...)
│       └── python/             # Python detection, venv management, subprocess execution
├── python/
│   ├── runner.py               # Build123d code executor (subprocess)
│   ├── manufacturing.py        # Mesh checks, orientation, sheet metal
│   └── evals/                  # Evaluation harness (80 test cases)
├── agent-rules/                # YAML presets (default, printing, cnc)
├── mechanisms/                 # Mechanism pattern catalog
└── model-iteration/            # Fine-tuning notes and test regression suite
```

---

## How It Works

```
User prompt → Rust backend builds system prompt from agent rules
            → Streams request to selected AI provider
            → SSE deltas forwarded to frontend via Tauri Channel
            → Python code block extracted from response
            → Displayed in Monaco editor
            → User executes (or auto-execute)
            → Build123d subprocess generates STL
            → STL base64-encoded → sent to frontend
            → Three.js renders interactive 3D model
            → On error: structured error + failed code sent back to AI for retry
```

---

## Evaluation

The project includes an offline evaluation harness with 80 test cases across categories: enclosure, mechanical, assembly, organic, sheet metal, printable, fixture, and robotics.

```bash
python python/evals/run_eval.py \
  --cases-dir python/evals/cases \
  --max-attempts 4
```

Quality gates:
- First-pass success: 55%
- Success within retry budget: 88%
- Manifold (watertight) geometry: 95%

---

## Docker

For a reproducible dev environment:

```bash
docker compose build cadai

# Install dependencies + run dev
docker compose run --service-ports --rm cadai bash -lc "pnpm install && pnpm tauri dev"

# Run evaluation harness
docker compose run --rm cadai bash -lc \
  "python python/evals/run_eval.py --cases-dir python/evals/cases --max-attempts 4"
```

---

## License

MIT
