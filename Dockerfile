FROM clux/muslrust:stable AS chef
USER root
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | sh
RUN cargo binstall cargo-chef -y
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --bin freecaster-grid --target x86_64-unknown-linux-musl

FROM alpine:3.21.3 AS runtime
RUN apk add --no-cache \
    ca-certificates \
    openssl \
    libssl3 \
    && rm -rf /var/cache/apk/*

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/freecaster-grid /usr/local/bin/
RUN chmod +x /usr/local/bin/freecaster-grid

RUN addgroup -S -g 1000 appgroup && adduser -S -u 1000 appuser -G appgroup
USER appuser

ENTRYPOINT ["/usr/local/bin/freecaster-grid", "/config/config.yaml"]
