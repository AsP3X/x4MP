# Human: Production image for x4mp-server (WebSocket session + event log).
# Agent: multi-stage build; stubs all workspace members for cargo cache; RUNS x4mp-server on 0.0.0.0:7878.
FROM rust:1-bookworm AS builder
WORKDIR /app

# Cache dependencies before copying full sources (workspace lists proto, server, bridge, tools/replay).
COPY Cargo.toml Cargo.lock ./
COPY proto/Cargo.toml proto/Cargo.toml
COPY server/Cargo.toml server/Cargo.toml
COPY bridge/Cargo.toml bridge/Cargo.toml
COPY tools/replay/Cargo.toml tools/replay/Cargo.toml
RUN mkdir -p proto/src server/src bridge/src tools/replay/src \
    && echo "pub fn lib_build_stub() {}" > proto/src/lib.rs \
    && echo "pub fn lib_build_stub() {}" > server/src/lib.rs \
    && echo "fn main() {}" > server/src/main.rs \
    && echo "pub fn lib_build_stub() {}" > bridge/src/lib.rs \
    && echo "fn main() {}" > bridge/src/main.rs \
    && echo "pub fn lib_build_stub() {}" > tools/replay/src/lib.rs \
    && echo "fn main() {}" > tools/replay/src/main.rs \
    && cargo build --release -p x4mp-server \
    && rm -rf proto/src server/src bridge/src tools/replay/src \
        target/release/.fingerprint/x4mp-* \
        target/release/deps/x4mp_* \
        target/release/x4mp-server \
        target/release/libx4mp_*

COPY proto proto
COPY server server
RUN cargo build --release -p x4mp-server

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/x4mp-server /usr/local/bin/x4mp-server

ENV X4MP_BIND_ADDR=0.0.0.0:7878
ENV X4MP_DATA_DIR=/data
ENV RUST_LOG=x4mp_server=info

EXPOSE 7878
VOLUME ["/data"]

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -fsS http://127.0.0.1:7878/health || exit 1

CMD ["x4mp-server"]
