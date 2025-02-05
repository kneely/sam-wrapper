FROM rust:latest as builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    git \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Install wasm32 target, component tooling, and rustfmt
RUN rustup target add wasm32-unknown-unknown && \
    rustup target add wasm32-wasip1 && \
    rustup component add rustfmt && \
    cargo install cargo-component --locked --version 0.13.2

# Copy source files
COPY . .

# Build wasm component
RUN cargo component build --release --target wasm32-unknown-unknown

# Final stage - just the wasm file
FROM scratch
COPY --from=builder /build/target/wasm32-unknown-unknown/release/*.wasm / 