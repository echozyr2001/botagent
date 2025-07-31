# -----------------------------------------------------------------------------
# ByteBot Agent Rust Service Dockerfile
# Multi-stage build for optimized production image
# -----------------------------------------------------------------------------

# -----------------------------------------------------------------------------
# Stage 1: Build stage
# -----------------------------------------------------------------------------
FROM rust:1.75-slim as builder

# Install system dependencies needed for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy workspace Cargo.toml and Cargo.lock first for better caching
COPY Cargo.toml Cargo.lock ./

# Copy shared library
COPY packages/bytebot-shared-rs/ ./packages/bytebot-shared-rs/

# Copy agent service source
COPY packages/bytebot-agent-rs/ ./packages/bytebot-agent-rs/

# Build the application in release mode
WORKDIR /app/packages/bytebot-agent-rs
RUN cargo build --release

# -----------------------------------------------------------------------------
# Stage 2: Runtime stage
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for security
RUN useradd -r -s /bin/false -m -d /app bytebot

# Create app directory and set ownership
WORKDIR /app
RUN chown bytebot:bytebot /app

# Copy the built binary from builder stage
COPY --from=builder /app/packages/bytebot-agent-rs/target/release/bytebot-agent-rs /usr/local/bin/bytebot-agent-rs

# Make binary executable
RUN chmod +x /usr/local/bin/bytebot-agent-rs

# Switch to non-root user
USER bytebot

# Expose the service port
EXPOSE 9991

# Health check configuration
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:9991/health || exit 1

# Set environment variables
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1

# Run the application
CMD ["bytebot-agent-rs"]