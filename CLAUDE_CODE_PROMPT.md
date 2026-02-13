# CAD AI Studio - Claude Code Context

## Overview

CAD AI Studio is an open source, model-agnostic AI-powered CAD application ("Windsurf for CAD"). Users chat with AI models to generate Build123d (Python) code that renders 3D models in real-time.

## Tech Stack

- **Desktop:** Tauri 2.x (Rust backend)
- **Frontend:** Svelte 5 + SvelteKit (adapter-static) + TypeScript
- **3D Rendering:** Three.js in Tauri webview
- **Code Editor:** Monaco Editor
- **CAD Engine:** Build123d via Python subprocess (not Rust-native)
- **AI:** REST APIs — Claude (SSE), OpenAI (SSE), Ollama (NDJSON)

## Architecture

- Single `src-tauri` crate with modules (not a Cargo workspace)
- Frontend in `src/` (Svelte 5 + TypeScript)
- Python subprocess for Build123d execution (`python/runner.py`)
- Agent rules in `agent-rules/*.yaml`

## Key Files

- `PLAN.md` — Full project plan and architecture
- `agent-rules/default.yaml` — AI behavior configuration (Build123d-focused)
- `src-tauri/src/lib.rs` — Tauri setup and module declarations
- `src-tauri/src/commands/` — Tauri IPC command handlers
- `src-tauri/src/ai/` — AI provider implementations
- `src-tauri/src/agent/` — Agent system (rules, prompts, validation)
- `src-tauri/src/python/` — Python/Build123d subprocess management
- `src/lib/components/` — Svelte 5 UI components
- `src/lib/services/viewport-engine.ts` — Three.js scene manager

## Data Flow

```
User message → Rust builds system prompt → AI provider (streaming) →
SSE deltas via Tauri Channel → Frontend chat updates →
Extract Python code → Monaco editor → Build123d subprocess →
STL output → Three.js renders in viewport
```

## Conventions

- Rust 2021 edition
- `thiserror` for error types
- `async-trait` for async traits
- `tokio` for async runtime
- `serde` + `serde_json`/`serde_yaml` for serialization
- Svelte 5 runes (`$state`, `$derived`, `$effect`)
- TypeScript strict mode
- Dark theme throughout

## Build123d Code Style (what AI generates)

```python
from build123d import *

# Mounting bracket for sensor
# Designed for 3D printing in PLA/PETG

THICKNESS = 3.0      # mm - wall thickness
BOLT_HOLE_DIA = 5.5  # mm - clearance for M5

with BuildPart() as p:
    Box(40, 30, THICKNESS)
    with BuildSketch(p.faces().sort_by(Axis.Z)[-1]):
        with Locations([(15, 10), (15, -10)]):
            Circle(BOLT_HOLE_DIA / 2)
    extrude(amount=-THICKNESS, mode=Mode.SUBTRACT)

result = p.part
```
