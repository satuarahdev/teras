FROM rust:slim-bookworm AS builder

# Install dependencies required for openssl/rustls
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app
# Copy manifests
COPY Cargo.toml Cargo.lock ./
# Create dummy src to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Copy actual source code and JSON resources used by include_str!
COPY src ./src
COPY endpoint_mapping.json gemini_tools.json ./
# Touch main.rs to force cargo to rebuild it
RUN touch src/main.rs
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

ENV TZ=Asia/Jakarta
RUN ln -snf /usr/share/zoneinfo/$TZ /etc/localtime && echo $TZ > /etc/timezone
RUN apt-get update && apt-get install -y tzdata libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /usr/src/app/target/release/teras /usr/local/bin/teras

CMD ["teras"]
