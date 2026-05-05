FROM rust:1.83-slim AS builder
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
RUN cargo build --release --bin lovely-server

FROM gcr.io/distroless/cc-debian12
COPY --from=builder /build/target/release/lovely-server /usr/local/bin/lovely-server
COPY static/ /opt/lovely/static/
COPY migrations/ /opt/lovely/migrations/
ENV LOVELY_STATIC_DIR=/opt/lovely/static
ENV LOVELY_SQLITE_DATA_DIR=/data/apps
USER nonroot:nonroot
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/lovely-server"]
