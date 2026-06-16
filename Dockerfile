# syntax=docker/dockerfile:1.7

# ---- Stage 1: build the frontend with Bun ----
FROM oven/bun:1 AS frontend-build
WORKDIR /app/frontend
COPY frontend/package.json frontend/bun.lock* frontend/bun.lockb* ./
RUN bun install --frozen-lockfile || bun install
COPY frontend/ ./
RUN bun run build

# ---- Stage 2: build the Rust binary ----
FROM rust:1-bookworm AS rust-build
WORKDIR /app

# Install build deps for any -sys crates we may transitively depend on.
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace manifests first for better layer caching.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY migrations ./migrations

# sqlx::migrate!("../../migrations") embeds migrations at compile time.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --bin xplan-server \
    && cp target/release/xplan-server /usr/local/bin/xplan-server

# ---- Stage 3: minimal runtime image ----
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /app xplan

WORKDIR /app
COPY --from=rust-build /usr/local/bin/xplan-server /usr/local/bin/xplan-server
COPY --from=frontend-build /app/frontend/dist ./frontend/dist
COPY config ./config
COPY migrations ./migrations

USER xplan
EXPOSE 26011
ENV XPLAN__SERVER__HOST=0.0.0.0 \
    XPLAN__SERVER__PORT=26011 \
    RUST_LOG=xplan=info,tower_http=info

ENTRYPOINT ["/usr/local/bin/xplan-server"]
