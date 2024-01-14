# 1. This tells docker to use the Rust official image
FROM rust:1.75.0

# 2. Copy the files in your machine to the Docker image
COPY content/ content/
COPY public/ public/
COPY src/ src/
COPY Cargo.lock Cargo.lock
COPY Cargo.toml Cargo.toml

# Build your program for release
RUN cargo build --release

# Run the binary
CMD ["./target/release/content_paulmcbride_com"]
