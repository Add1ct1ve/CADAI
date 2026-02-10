# Docker Workflow

This project now includes a reproducible Docker environment with:

- Node + npm + pnpm
- Rust toolchain (`cargo`, `rustc`, `clippy`, `rustfmt`)
- Python 3 (`python`, `python3`, `pip`, `venv`)
- Linux dependencies required by Tauri/WebKit

## 1) Build the image

```bash
docker compose build cadai
```

If your Docker Desktop setup has `buildx` issues, fallback to classic build:

```bash
DOCKER_BUILDKIT=0 docker build -t cadai-dev:latest .
```

## 2) Install JS dependencies inside the container

```bash
docker compose run --rm cadai bash -lc "pnpm install --frozen-lockfile"
```

## 3) Run quality checks (frontend + backend)

```bash
docker compose run --rm cadai bash -lc "pnpm install --frozen-lockfile && pnpm check && pnpm build && cd src-tauri && cargo test"
```

## 4) Run clippy (optional strict lint pass)

```bash
docker compose run --rm cadai bash -lc "pnpm install --frozen-lockfile && cd src-tauri && cargo clippy --all-targets --all-features"
```

## 5) Run Tauri dev inside container

```bash
docker compose run --service-ports --rm cadai bash -lc "pnpm install --frozen-lockfile && pnpm tauri dev"
```

Notes:

- `pnpm tauri dev` needs a GUI display on the host. In headless environments, use steps 3/4.
- The compose file mounts caches for `cargo`, `rustup`, and `pnpm` so repeated runs are faster.
