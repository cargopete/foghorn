# ── Build stage ────────────────────────────────────────────────────────────────
FROM rust:1-slim AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache dependency layer by copying manifests first and building with dummy sources
COPY Cargo.toml Cargo.lock ./
COPY crates/foghorn-core/Cargo.toml  crates/foghorn-core/Cargo.toml
COPY crates/foghorn-probe/Cargo.toml crates/foghorn-probe/Cargo.toml
COPY crates/foghorn-api/Cargo.toml   crates/foghorn-api/Cargo.toml

RUN mkdir -p crates/foghorn-core/src  \
             crates/foghorn-probe/src \
             crates/foghorn-api/src   && \
    echo "pub fn placeholder() {}" > crates/foghorn-core/src/lib.rs && \
    echo "fn main() {}"            > crates/foghorn-probe/src/main.rs && \
    echo "fn main() {}"            > crates/foghorn-api/src/main.rs && \
    cargo build --release 2>/dev/null || true && \
    rm -rf crates/foghorn-core/src crates/foghorn-probe/src crates/foghorn-api/src

# Real build
COPY crates crates
COPY migrations migrations
RUN touch crates/foghorn-core/src/lib.rs \
         crates/foghorn-probe/src/main.rs \
         crates/foghorn-api/src/main.rs && \
    cargo build --release

# ── Probe runtime ───────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS probe

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/foghorn-probe /usr/local/bin/foghorn-probe

EXPOSE 8080
CMD ["foghorn-probe"]

# ── API runtime ─────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS api

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/foghorn-api /usr/local/bin/foghorn-api

EXPOSE 8080
CMD ["foghorn-api"]
