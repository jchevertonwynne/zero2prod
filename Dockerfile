# stolen from https://github.com/eeff/zero2prod/blob/main/Dockerfile
FROM rust:1.53 AS builder
WORKDIR /app
RUN cargo install --locked --branch master \
    --git https://github.com/eeff/cargo-build-deps
COPY Cargo.toml Cargo.lock ./
RUN cargo build-deps --release
ENV SQLX_OFFLINE true
COPY . .
RUN cargo build --release --bin zero2prod


FROM debian:buster-slim AS runtime
WORKDIR /app
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends openssl \
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/zero2prod zero2prod
COPY configuration configuration
ENV APP_ENVIRONMENT production
ENTRYPOINT ["./zero2prod"]