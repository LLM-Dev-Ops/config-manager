# =============================================================================
# LLM-CONFIG-MANAGER DOCKERFILE
# Google Cloud Run / Edge Functions
# =============================================================================
#
# ARCHITECTURAL CONSTRAINTS:
#   - Stateless execution
#   - NO local persistence
#   - ALL data via ruvector-service
#   - Read-only configuration analysis
# =============================================================================

# Build stage
FROM rust:1.75-slim AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY agents ./agents

# Build release binary
RUN cargo build --release --workspace

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 1000 -g nogroup appuser

# Copy binaries from builder
COPY --from=builder /app/target/release/llm-config-server /usr/local/bin/
COPY --from=builder /app/target/release/llm-config-cli /usr/local/bin/

# Copy configuration templates
COPY config /etc/llm-config/templates

# Set ownership
RUN chown -R appuser:nogroup /app /etc/llm-config

# Switch to non-root user
USER appuser

# Environment defaults
ENV RUST_LOG=info,llm_config=debug
ENV SERVER_HOST=0.0.0.0
ENV SERVER_PORT=8080
ENV METRICS_PORT=9090

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Expose ports
EXPOSE 8080 9090

# Start server
ENTRYPOINT ["/usr/local/bin/llm-config-server"]
CMD ["serve"]
