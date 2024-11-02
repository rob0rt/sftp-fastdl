FROM rust:1.82 as builder

WORKDIR /app
COPY . .

RUN cargo build --release --bin app

FROM debian:bookworm-slim AS runtime

COPY --from=builder /app/target/release/app /usr/local/bin/app

ENTRYPOINT ["/usr/local/bin/app"]