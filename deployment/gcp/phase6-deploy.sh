#!/bin/bash
# Phase 6 Core Infrastructure Deployment Script
# Deploys deterministic agents to Google Cloud Run

set -euo pipefail

# ============================================
# Configuration
# ============================================

PROJECT_ID="${PROJECT_ID:-}"
REGION="${REGION:-us-central1}"
SERVICE_NAME="${SERVICE_NAME:-llm-config-manager}"
SERVICE_VERSION="${SERVICE_VERSION:-0.1.0}"
PLATFORM_ENV="${PLATFORM_ENV:-production}"

# Secrets (must exist in Secret Manager)
RUVECTOR_SERVICE_URL_SECRET="ruvector-service-url"
RUVECTOR_API_KEY_SECRET="ruvector-api-key"

# Performance budgets
MAX_TOKENS=800
MAX_LATENCY_MS=1500

# ============================================
# Validation
# ============================================

if [[ -z "$PROJECT_ID" ]]; then
    echo "ERROR: PROJECT_ID must be set"
    exit 1
fi

echo "============================================"
echo "Phase 6 Core Infrastructure Deployment"
echo "============================================"
echo "Project:     $PROJECT_ID"
echo "Region:      $REGION"
echo "Service:     $SERVICE_NAME"
echo "Version:     $SERVICE_VERSION"
echo "Environment: $PLATFORM_ENV"
echo "============================================"

# ============================================
# Verify Prerequisites
# ============================================

echo ""
echo "[1/5] Verifying prerequisites..."

# Check gcloud authentication
if ! gcloud auth print-access-token &>/dev/null; then
    echo "ERROR: Not authenticated with gcloud. Run 'gcloud auth login'"
    exit 1
fi

# Check secrets exist
for secret in "$RUVECTOR_SERVICE_URL_SECRET" "$RUVECTOR_API_KEY_SECRET"; do
    if ! gcloud secrets describe "$secret" --project="$PROJECT_ID" &>/dev/null; then
        echo "ERROR: Secret '$secret' not found in Secret Manager"
        echo "Create it with: gcloud secrets create $secret --project=$PROJECT_ID"
        exit 1
    fi
done

echo "  Prerequisites verified"

# ============================================
# Build Container Image
# ============================================

echo ""
echo "[2/5] Building container image..."

IMAGE_URI="gcr.io/$PROJECT_ID/$SERVICE_NAME:$SERVICE_VERSION"

# Build from agents directory
cd "$(dirname "$0")/../../agents"

gcloud builds submit \
    --project="$PROJECT_ID" \
    --tag="$IMAGE_URI" \
    --timeout=20m \
    .

echo "  Image built: $IMAGE_URI"

# ============================================
# Run Security Scan (Ruvector)
# ============================================

echo ""
echo "[3/5] Running security scan..."

# Container Analysis API scan
gcloud artifacts docker images scan "$IMAGE_URI" \
    --project="$PROJECT_ID" \
    --format="json" > /tmp/scan-results.json 2>/dev/null || true

CRITICAL_COUNT=$(jq '.vulnerabilities | map(select(.severity == "CRITICAL")) | length' /tmp/scan-results.json 2>/dev/null || echo "0")
HIGH_COUNT=$(jq '.vulnerabilities | map(select(.severity == "HIGH")) | length' /tmp/scan-results.json 2>/dev/null || echo "0")

echo "  Scan results: $CRITICAL_COUNT critical, $HIGH_COUNT high vulnerabilities"

if [[ "$CRITICAL_COUNT" -gt 0 ]]; then
    echo "WARNING: Critical vulnerabilities found. Review before production deployment."
fi

# ============================================
# Deploy to Cloud Run
# ============================================

echo ""
echo "[4/5] Deploying to Cloud Run..."

gcloud run deploy "$SERVICE_NAME" \
    --project="$PROJECT_ID" \
    --region="$REGION" \
    --image="$IMAGE_URI" \
    --platform=managed \
    --ingress=internal \
    --no-allow-unauthenticated \
    --service-account="${SERVICE_NAME}-sa@${PROJECT_ID}.iam.gserviceaccount.com" \
    --set-secrets="RUVECTOR_SERVICE_URL=${RUVECTOR_SERVICE_URL_SECRET}:latest,RUVECTOR_API_KEY=${RUVECTOR_API_KEY_SECRET}:latest" \
    --set-env-vars="PLATFORM_ENV=${PLATFORM_ENV},SERVICE_NAME=${SERVICE_NAME},SERVICE_VERSION=${SERVICE_VERSION},MAX_TOKENS=${MAX_TOKENS},MAX_LATENCY_MS=${MAX_LATENCY_MS},RUST_LOG=info" \
    --memory=512Mi \
    --cpu=1 \
    --min-instances=1 \
    --max-instances=10 \
    --timeout=60s \
    --concurrency=80 \
    --labels="phase=6,layer=core-infrastructure,version=${SERVICE_VERSION}"

echo "  Deployment complete"

# ============================================
# Verify Deployment
# ============================================

echo ""
echo "[5/5] Verifying deployment..."

SERVICE_URL=$(gcloud run services describe "$SERVICE_NAME" \
    --project="$PROJECT_ID" \
    --region="$REGION" \
    --format="value(status.url)")

echo "  Service URL: $SERVICE_URL"

# Get auth token for internal service
TOKEN=$(gcloud auth print-identity-token --audiences="$SERVICE_URL" 2>/dev/null || echo "")

if [[ -n "$TOKEN" ]]; then
    # Health check
    HEALTH_RESPONSE=$(curl -s -w "%{http_code}" -o /tmp/health.json \
        -H "Authorization: Bearer $TOKEN" \
        "${SERVICE_URL}/health" 2>/dev/null || echo "000")

    if [[ "$HEALTH_RESPONSE" == "200" ]]; then
        echo "  Health check: PASSED"
        cat /tmp/health.json
    else
        echo "  Health check: HTTP $HEALTH_RESPONSE"
    fi
fi

# ============================================
# Summary
# ============================================

echo ""
echo "============================================"
echo "Deployment Summary"
echo "============================================"
echo "Service:     $SERVICE_NAME"
echo "URL:         $SERVICE_URL"
echo "Version:     $SERVICE_VERSION"
echo ""
echo "DecisionEvent Signals:"
echo "  - config_validation_signal"
echo "  - schema_violation_signal"
echo "  - integration_health_signal"
echo ""
echo "Endpoints:"
echo "  - /health"
echo "  - /api/v1/validation/*  (config-validation)"
echo "  - /api/v1/schema/*      (schema-truth)"
echo "  - /api/v1/integration/* (integration-health)"
echo ""
echo "Performance Budgets:"
echo "  - MAX_TOKENS:     $MAX_TOKENS"
echo "  - MAX_LATENCY_MS: $MAX_LATENCY_MS"
echo "============================================"
echo ""
echo "Cloud Run Deploy Command Template:"
echo ""
echo "gcloud run deploy $SERVICE_NAME \\"
echo "    --project=\$PROJECT_ID \\"
echo "    --region=$REGION \\"
echo "    --image=$IMAGE_URI \\"
echo "    --platform=managed \\"
echo "    --ingress=internal \\"
echo "    --no-allow-unauthenticated \\"
echo "    --service-account=${SERVICE_NAME}-sa@\${PROJECT_ID}.iam.gserviceaccount.com \\"
echo "    --set-secrets=\"RUVECTOR_SERVICE_URL=${RUVECTOR_SERVICE_URL_SECRET}:latest,RUVECTOR_API_KEY=${RUVECTOR_API_KEY_SECRET}:latest\""
