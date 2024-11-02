FROM rust:1.82 as builder

WORKDIR /app
COPY . .

RUN cargo build --release --bin app

FROM debian:buster-slim AS runtime

WORKDIR /app
COPY --from=builder /app/target/release/app /usr/local/bin

ENTRYPOINT ["/usr/local/bin/app"]