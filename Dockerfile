FROM rust:1.85-slim AS builder

WORKDIR /app

COPY Cargo.lock Cargo.toml ./
COPY src/ src/

RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime

WORKDIR /app

ENV PORT=8000

RUN apt-get update \
    && apt-get install --yes --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --home-dir /app appuser

COPY --from=builder /app/target/release/web /usr/local/bin/web
COPY --chown=appuser:appuser content/ content/
COPY --chown=appuser:appuser public/ public/

USER appuser

EXPOSE 8000

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
    CMD curl --fail "http://127.0.0.1:${PORT}/health-check" || exit 1

CMD ["web"]
