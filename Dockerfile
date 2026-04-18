# Stage 1: Rust binary
FROM rust:1.88-slim-bookworm AS rust-builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Cache dependencies separately from source
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && \
    echo 'fn main(){}' > src/main.rs && \
    touch src/lib.rs && \
    cargo build --release && \
    rm -rf src

# Build the real binary (migrations dir required by sqlx::migrate! at compile time)
COPY migrations ./migrations
COPY src ./src
RUN touch src/main.rs src/lib.rs && cargo build --release

# Stage 2: UI build
FROM node:20-slim AS ui-builder
WORKDIR /ui
COPY ui/package.json ui/package-lock.json ./
RUN npm ci
COPY ui/ ./
RUN npm run build

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
