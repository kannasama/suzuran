# Phase 1.1 — Project Scaffold Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bootstrap the suzuran repository with a compiling Rust/Axum binary, a working 3-stage Dockerfile, and a docker-compose stack that returns 200 from `GET /health`.

**Architecture:** Single Rust binary (`suzuran-server`) with Axum + Tokio. Three-stage Docker build: Rust builder → UI builder placeholder → final Debian slim image. docker-compose provides Postgres and mounts a local `.env` file.

**Tech Stack:** Rust 1.78+, Axum 0.7, Tokio, tracing/tracing-subscriber, serde/serde_json, Docker multi-stage build, docker-compose v2.

---

## File Map

| File | Action | Purpose |
|------|--------|---------|
| `Cargo.toml` | Create | Workspace root + suzuran-server package |
| `src/main.rs` | Create | Entry point — loads config, builds router, starts server |
| `src/app.rs` | Create | Axum router construction |
| `Dockerfile` | Create | 3-stage build: rust-builder → ui-builder → final |
| `docker-compose.yml` | Create | App + Postgres services |
| `.env.example` | Create | Required env vars with safe defaults |
| `.dockerignore` | Create | Exclude target/, ui/node_modules, secrets/ |
| `tests/health.rs` | Create | Integration test: health endpoint returns 200 |

---

## Task 1: Cargo.toml and source skeleton

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/app.rs`

- [ ] **Step 1: Write `Cargo.toml`**

```toml
[package]
name = "suzuran-server"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "suzuran-server"
path = "src/main.rs"

[dependencies]
axum = "0.7"
tokio = { version = "1", features = ["full"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["trace"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
dotenvy = "0.15"

[dev-dependencies]
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1", features = ["full"] }
```

- [ ] **Step 2: Write `src/app.rs`**

```rust
use axum::{routing::get, Router};

pub fn build_router() -> Router {
    Router::new().route("/health", get(health))
}

async fn health() -> &'static str {
    "ok"
}
```

- [ ] **Step 3: Write `src/main.rs`**

```rust
mod app;

use anyhow::Context;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env if present (dev convenience; ignored in prod where vars are set directly)
    let _ = dotenvy::dotenv();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".into())
        .parse()
        .context("PORT must be a valid port number")?;

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    let router = app::build_router();

    tracing::info!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;
    Ok(())
}
```

- [ ] **Step 4: Verify it compiles (outside Docker for speed)**

```bash
cargo build 2>&1 | tail -5
```

Expected: `Finished dev [unoptimized + debuginfo] target(s)` — no errors.

- [ ] **Step 5: Commit**

```bash
git checkout -b feature/phase-1-scaffold
git add Cargo.toml src/
git commit -m "feat: initial Axum server with /health endpoint"
```

---

## Task 2: Integration test for /health

**Files:**
- Create: `tests/health.rs`

- [ ] **Step 1: Write the failing test**

```rust
// tests/health.rs
#[tokio::test]
async fn health_returns_ok() {
    // Bind to a random port to avoid conflicts
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, suzuran_server::build_router()).await.unwrap();
    });

    let res = reqwest::get(format!("http://{addr}/health"))
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    assert_eq!(res.text().await.unwrap(), "ok");
}
```

- [ ] **Step 2: Expose `build_router` as a library function**

Add to `Cargo.toml` under `[lib]`:

```toml
[lib]
name = "suzuran_server"
path = "src/lib.rs"
```

Create `src/lib.rs`:

```rust
mod app;
pub use app::build_router;
```

Update `src/main.rs` to use the lib:

```rust
use suzuran_server::build_router;
use anyhow::Context;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".into())
        .parse()
        .context("PORT must be a valid port number")?;

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    let router = build_router();

    tracing::info!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;
    Ok(())
}
```

- [ ] **Step 3: Run test to verify it passes**

```bash
cargo test health_returns_ok -- --nocapture
```

Expected: `test health_returns_ok ... ok`

- [ ] **Step 4: Commit**

```bash
git add src/lib.rs tests/health.rs Cargo.toml src/main.rs
git commit -m "test: integration test for /health endpoint"
```

---

## Task 3: .env.example and .dockerignore

**Files:**
- Create: `.env.example`
- Create: `.dockerignore`

- [ ] **Step 1: Write `.env.example`**

```bash
# Copy to .env and fill in values for local development
DATABASE_URL=postgres://suzuran:suzuran@localhost:5432/suzuran
JWT_SECRET=change-me-in-production-use-a-long-random-string
PORT=3000
LOG_LEVEL=info
```

- [ ] **Step 2: Write `.dockerignore`**

```
target/
ui/node_modules/
secrets/
.env
.git/
*.md
docs/
tasks/
tests/
```

- [ ] **Step 3: Commit**

```bash
git add .env.example .dockerignore
git commit -m "chore: add .env.example and .dockerignore"
```

---

## Task 4: Dockerfile

**Files:**
- Create: `Dockerfile`

- [ ] **Step 1: Write `Dockerfile`**

```dockerfile
# Stage 1: Rust binary
FROM rust:1.78-slim-bookworm AS rust-builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Cache dependencies separately from source
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main(){}' > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Build the real binary
COPY src ./src
RUN touch src/main.rs && cargo build --release

# Stage 2: UI build (placeholder — fleshed out in Phase 1.10)
FROM node:20-slim AS ui-builder
WORKDIR /ui
RUN mkdir -p dist

# Stage 3: Final image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    ffmpeg \
    && rm -rf /var/lib/apt/lists/*

# Install fpcalc (chromaprint)
RUN apt-get update && apt-get install -y --no-install-recommends \
    libchromaprint-tools \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=rust-builder /build/target/release/suzuran-server ./
COPY --from=ui-builder /ui/dist ./ui/dist

ENV PORT=3000
ENV LOG_LEVEL=info

EXPOSE 3000

CMD ["./suzuran-server"]
```

> **Note on fpcalc:** The `libchromaprint-tools` package provides `fpcalc` on Debian bookworm. Verify with `docker run --rm <image> which fpcalc` after building.

- [ ] **Step 2: Verify the Docker build**

```bash
docker buildx build --progress=plain -t suzuran:dev .
```

Expected: All 3 stages complete, image tagged `suzuran:dev`. Final binary should be ~10–20 MB stripped.

- [ ] **Step 3: Verify the binary runs**

```bash
docker run --rm -e JWT_SECRET=test -e DATABASE_URL=postgres://x suzuran:dev ./suzuran-server &
sleep 2
curl -s http://localhost:3000/health
```

Expected output: `ok`

- [ ] **Step 4: Commit**

```bash
git add Dockerfile
git commit -m "chore: 3-stage Dockerfile (rust-builder, ui-builder placeholder, final)"
```

---

## Task 5: docker-compose

**Files:**
- Create: `docker-compose.yml`

- [ ] **Step 1: Write `docker-compose.yml`**

```yaml
services:
  db:
    image: postgres:16-alpine
    restart: unless-stopped
    environment:
      POSTGRES_USER: suzuran
      POSTGRES_PASSWORD: suzuran
      POSTGRES_DB: suzuran
    ports:
      - "5432:5432"
    volumes:
      - postgres_data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U suzuran"]
      interval: 5s
      timeout: 5s
      retries: 5

  app:
    build: .
    restart: unless-stopped
    ports:
      - "${PORT:-3000}:3000"
    environment:
      DATABASE_URL: postgres://suzuran:suzuran@db:5432/suzuran
      JWT_SECRET: ${JWT_SECRET:-dev-secret-change-in-prod}
      PORT: 3000
      LOG_LEVEL: ${LOG_LEVEL:-info}
    depends_on:
      db:
        condition: service_healthy

volumes:
  postgres_data:
```

- [ ] **Step 2: Bring up the stack**

```bash
docker compose up --build -d
```

- [ ] **Step 3: Verify health endpoint through docker-compose**

```bash
sleep 3
curl -s http://localhost:3000/health
```

Expected: `ok`

- [ ] **Step 4: Tear down**

```bash
docker compose down
```

- [ ] **Step 5: Commit**

```bash
git add docker-compose.yml
git commit -m "chore: docker-compose with Postgres and app service"
```

---

## Task 6: Update CLAUDE.md with build commands

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Replace "No source code yet" project status section with actual commands**

Find the `## Project Status` section in `CLAUDE.md` and replace it with:

```markdown
## Build & Test Commands

```bash
# Local (requires Rust toolchain)
cargo build                          # debug build
cargo test                           # all tests
cargo test <name>                    # single test

# Docker (canonical build — use this to verify before committing)
docker buildx build --progress=plain -t suzuran:dev .
docker compose up --build -d         # start full stack
docker compose down                  # stop stack
docker compose logs -f app           # follow app logs
```
```

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: add build and test commands to CLAUDE.md"
```

---

## Task 7: Update codebase filemap

**Files:**
- Modify: `tasks/codebase-filemap.md`

- [ ] **Step 1: Add all new files to the filemap**

Add entries for: `Cargo.toml`, `src/lib.rs`, `src/main.rs`, `src/app.rs`, `Dockerfile`, `docker-compose.yml`, `.env.example`, `.dockerignore`, `tests/health.rs`.

- [ ] **Step 2: Commit**

```bash
git add tasks/codebase-filemap.md
git commit -m "docs: update filemap for Phase 1.1 scaffold"
```

---

## Verification

After all tasks, the full stack test:

```bash
docker compose up --build -d
sleep 5
curl -f http://localhost:3000/health && echo "PASS"
docker compose down
```

Expected: `ok` then `PASS` — no errors.
