# Multi-stage build for MySQL MCP Server
FROM rust:latest as builder

# Install system dependencies for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml ./

# Copy source code
COPY src ./src

# Build for release
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies including curl for health checks
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -s /bin/false -m -d /app mcp

# Set working directory
WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/mysql-mcp-server /usr/local/bin/mysql-mcp-server

# Copy configuration templates
COPY config.example.toml ./config.example.toml
COPY config.multi-env.example.toml ./config.multi-env.example.toml
COPY config.docker.example.toml ./config.docker.example.toml

# Create config directory and set permissions
RUN mkdir -p /app/config && \
    chown -R mcp:mcp /app

# Switch to non-root user
USER mcp

# Expose port
EXPOSE 8080

# Enhanced health check that works with multi-environment setup
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Set default environment variables
ENV RUST_LOG=info
ENV PORT=8080
ENV CONFIG_FILE=/app/config.toml

# Support for environment variable configuration override
ENV MCP_DEFAULT_ENVIRONMENT=""
ENV MCP_SERVER_PORT=""
ENV MCP_LOG_LEVEL=""

# Run the binary
CMD ["mysql-mcp-server"]