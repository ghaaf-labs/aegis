# ── Build stage ───────────────────────────────────────────────────────────
FROM rust:1.94-slim-bookworm AS builder
ARG API_CARGO_FEATURES="real-cctp real-usyc"

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release --features "$API_CARGO_FEATURES" 2>/dev/null; rm -rf src

# Build application
COPY . .
RUN touch src/main.rs && cargo build --release --features "$API_CARGO_FEATURES"

# ── Runtime stage ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/aegis-api .
COPY --from=builder /app/migrations ./migrations

EXPOSE 8080

ENTRYPOINT ["./aegis-api"]
