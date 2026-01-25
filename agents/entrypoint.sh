#!/bin/sh
# Entrypoint for Phase 6 Core Infrastructure Agents
# Routes to appropriate agent based on command or environment

set -e

# Default agent if not specified
AGENT="${1:-config-validate}"

case "$AGENT" in
    config-validate|config-validation)
        shift 2>/dev/null || true
        exec /usr/local/bin/config-validate "$@"
        ;;
    schema-truth|schema)
        shift 2>/dev/null || true
        exec /usr/local/bin/schema-truth "$@"
        ;;
    integration-health|health)
        shift 2>/dev/null || true
        exec /usr/local/bin/integration-health "$@"
        ;;
    serve-all)
        # Run all agents (for unified service)
        # This uses a simple port multiplexer
        echo "Starting unified agent service..."
        /usr/local/bin/config-validate serve --port 8080 &
        /usr/local/bin/schema-truth serve --port 8081 &
        /usr/local/bin/integration-health serve --port 8082 &
        wait
        ;;
    *)
        # Assume it's a direct command
        exec "$@"
        ;;
esac
