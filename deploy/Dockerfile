FROM rust:slim AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --locked
COPY rust-toolchain.toml ./
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --bin lovely-server
COPY . .
RUN cargo build --jobs 1 --release --bin lovely-server

FROM debian:12-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/lovely-server /usr/local/bin/lovely-server
COPY static/ /opt/lovely/static/
COPY deploy/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh
ENV LOVELY_STATIC_DIR=/opt/lovely/static
ENV LOVELY_SQLITE_DATA_DIR=/data/apps
EXPOSE 8080
ENTRYPOINT ["/entrypoint.sh"]
CMD ["/usr/local/bin/lovely-server"]
