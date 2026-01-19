FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p lucidity-relay

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/lucidity-relay /usr/local/bin/
EXPOSE 9090
ENV LUCIDITY_RELAY_LISTEN=0.0.0.0:9090
CMD ["lucidity-relay"]
