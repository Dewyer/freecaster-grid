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
RUN cargo chef cook --release --target $(uname -p)-unknown-linux-musl --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --bin freecaster-grid --target $(uname -p)-unknown-linux-musl && \
    mv target/$(uname -p)-unknown-linux-musl/release/freecaster-grid freecaster-grid

FROM alpine:3.21.3 AS runtime

RUN apk add --no-cache \
    ca-certificates \
    su-exec \
    && rm -rf /var/cache/apk/*

COPY --from=builder /app/freecaster-grid /usr/local/bin/
RUN chmod +x /usr/local/bin/freecaster-grid

COPY docker-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["/usr/local/bin/freecaster-grid", "/config/config.yaml"]
