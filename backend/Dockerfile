# Stage 1: Builder — compile the application
FROM rust:latest AS builder
WORKDIR /app
COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release

# Stage 2: Runtime — minimal production image
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/verigate-backend /usr/local/bin/verigate-backend
COPY --from=builder /app/policies /app/policies
COPY --from=builder /app/migrations /app/migrations
COPY --from=builder /app/config /app/config

EXPOSE 3001

CMD ["verigate-backend"]
