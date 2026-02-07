# CAD AI Studio - Project Plan

> "Windsurf for CAD" - Open source, model-agnostic AI-powered CAD

## Vision

Desktop CAD application where users can:
1. Chat with any AI model (Claude, GPT, Ollama)
2. AI generates CadQuery (Python) code that instantly renders
3. Iterate visually until design is correct
4. Export to STL/STEP for manufacturing

## Tech Stack

| Component | Technology | Version |
|-----------|-----------|---------|
| Desktop Shell | Tauri 2.x | 2.10+ |
| Frontend Framework | Svelte 5 + SvelteKit (adapter-static) | 5.25+ |
| 3D Rendering | Three.js in webview | 0.182+ |
| Code Editor | Monaco Editor | 0.55+ |
| CAD Engine | CadQuery via Python subprocess | 2.4+ |
| AI Provider | REST APIs (Claude, OpenAI, Ollama) | — |
| HTTP Client | reqwest | 0.13 |
| Async Runtime | tokio | 1.43+ |
| Package Manager | pnpm | — |
| Platforms | Windows + macOS | — |

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                   TAURI 2.x SHELL                    │
├──────────────────────┬──────────────────────────────┤
│  FRONTEND (Svelte 5) │     RUST BACKEND             │
│                      │                              │
│  Chat Panel          │  AI Provider (trait-based)    │
│  Monaco Editor       │  ├─ Claude (SSE streaming)   │
│  Three.js Viewport   │  ├─ OpenAI                   │
│  Settings Panel      │  └─ Ollama (local)           │
│  Toolbar + StatusBar │                              │
│                      │  Agent System                │
│   ◄── Tauri IPC ──►  │  ├─ YAML rule loader         │
│  (Channel streaming)  │  ├─ System prompt builder    │
│                      │  └─ Validation pipeline      │
│                      │                              │
│                      │  Python Manager              │
│                      │  ├─ Auto-detect/installer    │
│                      │  ├─ Venv management          │
│                      │  └─ CadQuery subprocess      │
└──────────────────────┴──────────────────────────────┘
                              │
                    ┌─────────┴──────────┐
                    │  Python Subprocess  │
                    │  CadQuery → STL     │
                    └────────────────────┘
```

## Data Flow

1. User writes message in chat
2. Rust backend builds system prompt from agent-rules YAML
3. Sends to AI provider (Claude/OpenAI/Ollama) with streaming
4. SSE deltas streamed to frontend via Tauri Channel → chat updates live
5. When complete: extract ```python code block from AI response
6. Code shown in Monaco editor
7. CadQuery code written to temp file, run via Python subprocess
8. STL output read and sent to frontend
9. Three.js renders STL with orbit controls, lights, grid
10. On error: send error message back to AI for auto-retry (Phase 4)

## File Structure

```
cadai/
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/default.json
│   ├── icons/
│   └── src/
│       ├── main.rs
│       ├── lib.rs                  # Tauri setup + module declarations
│       ├── state.rs                # AppState (Mutex-wrapped)
│       ├── error.rs                # AppError (thiserror)
│       ├── config.rs               # AppConfig (serde, persisted)
│       ├── commands/
│       │   ├── mod.rs
│       │   ├── chat.rs             # send_message (streaming pipeline)
│       │   ├── cad.rs              # execute_code, export_stl, export_step
│       │   ├── settings.rs         # get/update settings, check_python
│       │   └── project.rs          # save/load project files
│       ├── ai/
│       │   ├── mod.rs
│       │   ├── provider.rs         # AiProvider trait
│       │   ├── claude.rs           # Anthropic API + SSE
│       │   ├── openai.rs           # OpenAI API + SSE
│       │   ├── ollama.rs           # Localhost NDJSON
│       │   ├── message.rs          # ChatMessage, Conversation
│       │   └── streaming.rs        # Shared SSE parser
│       ├── agent/
│       │   ├── mod.rs
│       │   ├── rules.rs            # AgentRules struct + YAML loader
│       │   ├── prompts.rs          # System prompt builder
│       │   ├── validate.rs         # Pre/post validation
│       │   └── context.rs          # Conversation context handling
│       └── python/
│           ├── mod.rs
│           ├── detector.rs         # Find Python on system
│           ├── installer.rs        # Auto-installer (venv + pip)
│           ├── venv.rs             # Venv management
│           └── runner.rs           # CadQuery subprocess executor
│
├── src/                            # Frontend (Svelte 5 + TS)
│   ├── app.html
│   ├── app.css
│   ├── lib/
│   │   ├── components/
│   │   │   ├── Viewport.svelte     # Three.js 3D viewport
│   │   │   ├── Chat.svelte         # Chat panel with streaming
│   │   │   ├── ChatMessage.svelte
│   │   │   ├── CodeEditor.svelte   # Monaco editor
│   │   │   ├── Settings.svelte
│   │   │   ├── Toolbar.svelte
│   │   │   ├── StatusBar.svelte
│   │   │   └── SplitPane.svelte    # Resizable panels
│   │   ├── stores/
│   │   │   ├── chat.ts
│   │   │   ├── viewport.ts
│   │   │   ├── settings.ts
│   │   │   └── project.ts
│   │   ├── services/
│   │   │   ├── tauri.ts            # Typed invoke() wrappers
│   │   │   └── viewport-engine.ts  # Three.js scene manager
│   │   └── types/index.ts
│   └── routes/
│       ├── +layout.svelte
│       └── +page.svelte
│
├── python/
│   ├── runner.py                   # CadQuery execution wrapper
│   └── requirements.txt            # cadquery>=2.4.0
│
├── agent-rules/
│   ├── default.yaml                # Standard agent rules
│   ├── printing-focused.yaml       # 3D printing optimized
│   └── cnc-focused.yaml            # CNC optimized
│
├── package.json
├── pnpm-lock.yaml
├── svelte.config.js
├── vite.config.ts
├── tsconfig.json
└── PLAN.md
```

## Development Phases

### Phase 1: Tauri Shell + Three.js Viewport + UI Layout
**Goal:** App starts, shows 3D viewport with a hardcoded cube, panels are resizable.

- [x] Scaffold Tauri 2 + Svelte 5
- [x] Configure adapter-static in SvelteKit
- [x] Install Three.js, build ViewportEngine class
- [x] Build three-panel layout: Chat | Viewport | Code Editor
- [x] Dark theme (IDE aesthetic)
- [x] StatusBar and Toolbar (placeholder buttons)
- [x] Verify Tauri IPC with a test command

### Phase 2: Python/CadQuery Integration
**Goal:** User writes CadQuery code in editor, clicks "Run", sees result in viewport.

- [ ] Python detector (python/python3/py -3 on Win, python3 on Mac)
- [ ] Venv creation in app data dir
- [ ] pip install cadquery in venv
- [ ] Python runner script (exec CadQuery, export STL)
- [ ] Rust subprocess executor
- [ ] execute_code Tauri command
- [ ] Monaco Editor with Python syntax highlighting
- [ ] Wire "Run" button → execute → viewport render

### Phase 3: AI Provider + Chat + Code Generation
**Goal:** User chats, AI streams CadQuery code, it runs automatically, model renders.

- [ ] AiProvider trait with complete() and stream()
- [ ] Claude API implementation with SSE streaming
- [ ] OpenAI API implementation
- [ ] Ollama implementation (NDJSON)
- [ ] Agent rules YAML parser
- [ ] System prompt builder
- [ ] send_message command with Tauri Channel streaming
- [ ] Chat.svelte with streaming messages
- [ ] Full pipeline: message → AI → code → CadQuery → STL → viewport

### Phase 4: Agent Intelligence
**Goal:** Auto error correction, validation, iterative design.

- [ ] Pre/post validation pipeline
- [ ] Auto-retry on CadQuery errors (max N attempts)
- [ ] Conversation context management
- [ ] Structured traceback parsing
- [ ] "Explain error" and "Retry" buttons
- [ ] Code diff display

### Phase 5: Polish
**Goal:** Production-ready app with settings, persistence, export.

- [ ] Settings panel (API keys, model selection, provider switching)
- [ ] Settings persistence (JSON in app data dir)
- [ ] Project save/load (conversation + code + settings)
- [ ] STL/STEP export with file dialog
- [ ] Agent rule presets
- [ ] Keyboard shortcuts
- [ ] App icon and branding

## Key Dependencies (Rust)

```toml
[dependencies]
tauri = "2.10"
tauri-plugin-shell = "2.2"
tauri-plugin-fs = "2.2"
tauri-plugin-dialog = "2.2"
tokio = { version = "1.43", features = ["full"] }
reqwest = { version = "0.13", features = ["json", "stream"] }
futures-util = "0.3"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"
async-trait = "0.1"
thiserror = "2.0"
uuid = { version = "1.0", features = ["v4"] }
regex = "1.11"
base64 = "0.22"
dirs = "6.0"
```

## Key Dependencies (Frontend)

```json
{
  "three": "^0.182.0",
  "@tauri-apps/api": "^2.5.0",
  "monaco-editor": "^0.55.1",
  "svelte": "^5.25.0",
  "@sveltejs/kit": "^2.20.0"
}
```

## Risks & Mitigations

| Risk | Level | Mitigation |
|------|-------|------------|
| Python auto-install fails | High | Graceful degradation, clear manual install instructions |
| CadQuery subprocess slow (2-5s cold start) | Medium | Warm process pool (Phase 4+), progress spinner |
| AI spatial confusion | Medium | Agent rules YAML, auto-retry with error context |
| STL base64 overhead | Low | Use raw bytes via Tauri invoke (Phase 5) |
| Three.js rendering quality | Low | Mature lib, MeshPhysicalMaterial, computeVertexNormals() |
