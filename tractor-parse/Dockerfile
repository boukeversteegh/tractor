FROM rust:1.83-slim

# Install cross-compilation toolchains
RUN apt-get update && apt-get install -y \
    mingw-w64 \
    && rm -rf /var/lib/apt/lists/*

# Add Windows target
RUN rustup target add x86_64-pc-windows-gnu

WORKDIR /src

# Copy source
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Default: build for Linux
CMD ["cargo", "build", "--release"]
