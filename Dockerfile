FROM rust:1.87-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY backend ./backend
RUN cargo build --release -p polymarket-backend

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --shell /usr/sbin/nologin appuser

WORKDIR /app
COPY --from=builder /app/target/release/polymarket-backend /usr/local/bin/polymarket-backend

USER appuser
EXPOSE 8080
ENV APP_HOST=0.0.0.0
ENV APP_PORT=8080

CMD ["polymarket-backend"]
