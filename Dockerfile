FROM rust:1.98-bookworm AS build
WORKDIR /src
RUN cargo install topcoat-cli --version 0.10.0
COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
RUN cargo build --release --bin catalog-bench \
    && topcoat asset bundle --release --bin catalog-bench

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /src/target/release/catalog-bench /app/catalog-bench
COPY --from=build /src/target/release/assets /app/assets
ENV HOST=0.0.0.0 \
    PORT=3000 \
    CATALOG_MB=1024
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=5s --start-period=180s --retries=3 \
    CMD curl -f http://127.0.0.1:3000/api/health || exit 1
CMD ["./catalog-bench"]
