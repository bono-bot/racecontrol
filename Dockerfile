# ── Stage 1: Build ───────────────────────────────────────────
FROM rust:bookworm AS builder

WORKDIR /app

# Install build deps for SQLite + OpenSSL (reqwest native-tls)
RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

# Cache dependencies by copying manifests first
COPY Cargo.toml Cargo.lock ./
COPY crates/rc-common/Cargo.toml crates/rc-common/Cargo.toml
COPY crates/racecontrol/Cargo.toml crates/racecontrol/Cargo.toml
COPY crates/rc-agent/Cargo.toml crates/rc-agent/Cargo.toml

# Create dummy source files to build deps
RUN mkdir -p crates/rc-common/src && echo "// dummy" > crates/rc-common/src/lib.rs \
    && mkdir -p crates/racecontrol/src && echo "fn main() {}" > crates/racecontrol/src/main.rs \
    && mkdir -p crates/rc-agent/src && echo "fn main() {}" > crates/rc-agent/src/main.rs

# Build deps only (cached layer)
RUN cargo build --release --package racecontrol 2>/dev/null || true

# Copy real source and assets
COPY crates/ crates/
COPY assets/ assets/

# Touch source files to invalidate cache for real build
RUN touch crates/rc-common/src/lib.rs crates/racecontrol/src/main.rs

# Build the actual binary
RUN cargo build --release --package racecontrol

# ── Stage 2: Runtime ─────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates libssl3 libsqlite3-0 curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/racecontrol /app/racecontrol

# Data directory for SQLite
RUN mkdir -p /app/data

EXPOSE 8080

HEALTHCHECK --interval=10s --timeout=5s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:8080/api/v1/health || exit 1

CMD ["/app/racecontrol"]
