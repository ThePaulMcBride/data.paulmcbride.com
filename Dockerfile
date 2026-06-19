FROM rust:1.85-slim AS builder

WORKDIR /app

COPY Cargo.lock Cargo.toml ./
COPY src/ src/

RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime

WORKDIR /app

ENV PORT=8000

RUN useradd --uid 10001 --home-dir /app appuser

COPY --from=builder /app/target/release/content_paulmcbride_com /usr/local/bin/content_paulmcbride_com
COPY --chown=appuser:appuser content/ content/
COPY --chown=appuser:appuser public/ public/

USER appuser

EXPOSE 8000

CMD ["content_paulmcbride_com"]
