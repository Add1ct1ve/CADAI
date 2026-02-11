# Docker Workflow (Validated)

This Docker setup is intended to make CADAI reproducible across machines.

Included toolchain:
- Node + npm + pnpm (`pnpm@10.29.2`)
- Rust toolchain (`cargo`, `rustc`, `clippy`, `rustfmt`)
- Python 3 (`python`, `python3`, `pip`, `venv`)
- Linux libs required by Tauri/WebKit

Persistent caches/volumes:
- `/workspace/node_modules` (container-only deps, avoids host/arch mismatch)
- Cargo registry/git
- Rustup
- pnpm store

## 1) Build image

```bash
docker compose build cadai
```

Fallback if BuildKit has issues:

```bash
DOCKER_BUILDKIT=0 docker build -t cadai-dev:latest .
```

## 2) Install dependencies

```bash
docker compose run --rm cadai bash -lc "pnpm install --frozen-lockfile"
```

## 3) Run the same checks used in this stabilization pass

```bash
docker compose run --rm cadai bash -lc "pnpm install --frozen-lockfile && pnpm check && pnpm build && cd src-tauri && cargo test"
```

## 3b) Run CAD generation eval harness (80 cases)

```bash
docker compose run --rm cadai bash -lc "pnpm install --frozen-lockfile && python python/evals/run_eval.py --cases-dir python/evals/cases --python-bin python --runner python/runner.py --manufacturing python/manufacturing.py --max-attempts 4"
```

If you want live generation evaluation, pass a generator adapter command:

```bash
docker compose run --rm cadai bash -lc \"python python/evals/run_eval.py --cases-dir python/evals/cases --generator-cmd './tools/generate_code.sh {prompt_file} {attempt}' --max-attempts 4\"
```

## 4) Run clippy (known pre-existing failure)

```bash
docker compose run --rm cadai bash -lc "pnpm install --frozen-lockfile && cd src-tauri && cargo clippy --all-targets --all-features"
```

Current expected status:
- `cargo clippy` fails on existing repo issue in `src-tauri/src/agent/confidence.rs:406` (`absurd_extreme_comparisons`).
- Other clippy warnings are non-blocking.

## 5) Run app in dev mode

```bash
docker compose run --service-ports --rm cadai bash -lc "pnpm install --frozen-lockfile && pnpm tauri dev"
```

For browser smoke-test only (without launching native window):

```bash
docker compose run --service-ports --rm cadai bash -lc "pnpm install --frozen-lockfile && pnpm dev --host 0.0.0.0 --port 4173"
```

Notes:
- `pnpm tauri dev` needs a GUI display on host.
- Compose enables file-watch polling for more reliable hot-reload on mounted volumes.
