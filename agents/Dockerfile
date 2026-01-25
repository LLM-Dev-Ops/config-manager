# Multi-stage build for Phase 6 Core Infrastructure Agents
# Agents: config-validation, schema-truth, integration-health

# ============================================
# Stage 1: Builder
# ============================================
FROM rust:1.83-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Build config-validation agent
COPY config-validation ./config-validation
WORKDIR /build/config-validation
RUN cargo build --release --bin config-validate

# Build schema-truth agent
WORKDIR /build
COPY schema-truth ./schema-truth
WORKDIR /build/schema-truth
RUN cargo build --release --bin schema-truth

# Build integration-health agent
WORKDIR /build
COPY integration-health ./integration-health
WORKDIR /build/integration-health
RUN cargo build --release --bin integration-health

# ============================================
# Stage 2: Runtime Image
# ============================================
FROM debian:bookworm-slim AS runtime

# Labels
LABEL org.opencontainers.image.title="LLM Config Manager - Phase 6 Core Infrastructure"
LABEL org.opencontainers.image.description="Deterministic configuration agents with DecisionEvent emission"
LABEL org.opencontainers.image.version="0.1.0"
LABEL org.opencontainers.image.vendor="LLM DevOps Team"
LABEL phase="6"
LABEL layer="core-infrastructure"

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -s /bin/false appuser

# Copy binaries
COPY --from=builder /build/config-validation/target/release/config-validate /usr/local/bin/
COPY --from=builder /build/schema-truth/target/release/schema-truth /usr/local/bin/
COPY --from=builder /build/integration-health/target/release/integration-health /usr/local/bin/

# Environment defaults
ENV RUST_LOG=info
ENV PORT=8080
ENV RUVECTOR_SERVICE_URL=""
ENV RUVECTOR_API_KEY=""
ENV PLATFORM_ENV=production
ENV SERVICE_NAME=llm-config-manager
ENV SERVICE_VERSION=0.1.0

# Performance budgets
ENV MAX_TOKENS=800
ENV MAX_LATENCY_MS=1500

# Run as non-root
USER appuser

# Expose port
EXPOSE 8080

# Default to schema-truth serve (has HTTP server capability)
CMD ["/usr/local/bin/schema-truth", "serve", "--port", "8080"]
