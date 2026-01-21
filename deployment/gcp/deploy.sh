#!/bin/bash
# =============================================================================
# LLM-CONFIG-MANAGER DEPLOYMENT SCRIPT
# Google Cloud Run Deployment
# =============================================================================

set -euo pipefail

# ===========================================
# Configuration
# ===========================================
PROJECT_ID="${PROJECT_ID:?PROJECT_ID environment variable required}"
REGION="${REGION:-us-central1}"
SERVICE_NAME="llm-config-manager"
SERVICE_VERSION="${SERVICE_VERSION:-1.0.0}"
PLATFORM_ENV="${PLATFORM_ENV:-production}"

echo "=============================================="
echo "LLM-CONFIG-MANAGER DEPLOYMENT"
echo "=============================================="
echo "Project:     $PROJECT_ID"
echo "Region:      $REGION"
echo "Service:     $SERVICE_NAME"
echo "Version:     $SERVICE_VERSION"
echo "Environment: $PLATFORM_ENV"
echo "=============================================="

# ===========================================
# Pre-flight Checks
# ===========================================
echo ""
echo "[1/8] Running pre-flight checks..."

# Check gcloud authentication
if ! gcloud auth list --filter=status:ACTIVE --format="value(account)" | head -1; then
    echo "ERROR: Not authenticated with gcloud. Run: gcloud auth login"
    exit 1
fi

# Check project access
if ! gcloud projects describe "$PROJECT_ID" &>/dev/null; then
    echo "ERROR: Cannot access project $PROJECT_ID"
    exit 1
fi

# Verify required APIs are enabled
for api in run.googleapis.com secretmanager.googleapis.com cloudbuild.googleapis.com; do
    if ! gcloud services list --project="$PROJECT_ID" --filter="name:$api" --format="value(name)" | grep -q "$api"; then
        echo "Enabling $api..."
        gcloud services enable "$api" --project="$PROJECT_ID"
    fi
done

echo "✓ Pre-flight checks passed"

# ===========================================
# IAM Setup
# ===========================================
echo ""
echo "[2/8] Setting up IAM..."

bash "$(dirname "$0")/iam-setup.sh"

echo "✓ IAM setup complete"

# ===========================================
# Build Image
# ===========================================
echo ""
echo "[3/8] Building container image..."

gcloud builds submit \
    --project="$PROJECT_ID" \
    --config="$(dirname "$0")/cloudbuild.yaml" \
    --substitutions="_SERVICE_VERSION=$SERVICE_VERSION,_PLATFORM_ENV=$PLATFORM_ENV,_REGION=$REGION" \
    "$(dirname "$0")/../.."

echo "✓ Container image built"

# ===========================================
# Deploy Service
# ===========================================
echo ""
echo "[4/8] Deploying to Cloud Run..."

gcloud run deploy "$SERVICE_NAME" \
    --project="$PROJECT_ID" \
    --region="$REGION" \
    --image="gcr.io/$PROJECT_ID/$SERVICE_NAME:$SERVICE_VERSION" \
    --platform=managed \
    --ingress=internal \
    --no-allow-unauthenticated \
    --service-account="${SERVICE_NAME}-sa@${PROJECT_ID}.iam.gserviceaccount.com" \
    --cpu=2 \
    --memory=2Gi \
    --min-instances=1 \
    --max-instances=100 \
    --concurrency=100 \
    --timeout=300 \
    --cpu-boost \
    --execution-environment=gen2 \
    --set-env-vars="SERVICE_NAME=$SERVICE_NAME,SERVICE_VERSION=$SERVICE_VERSION,PLATFORM_ENV=$PLATFORM_ENV" \
    --set-secrets="RUVECTOR_SERVICE_URL=llm-config-ruvector-url:latest,RUVECTOR_API_KEY=llm-config-ruvector-key:latest"

echo "✓ Service deployed"

# ===========================================
# Get Service URL
# ===========================================
echo ""
echo "[5/8] Retrieving service URL..."

SERVICE_URL=$(gcloud run services describe "$SERVICE_NAME" \
    --project="$PROJECT_ID" \
    --region="$REGION" \
    --format='value(status.url)')

echo "Service URL: $SERVICE_URL"

# ===========================================
# Verify Health
# ===========================================
echo ""
echo "[6/8] Verifying service health..."

TOKEN=$(gcloud auth print-identity-token)

for i in {1..30}; do
    HEALTH=$(curl -s -o /dev/null -w "%{http_code}" \
        -H "Authorization: Bearer $TOKEN" \
        "$SERVICE_URL/health" 2>/dev/null || echo "000")

    if [ "$HEALTH" = "200" ]; then
        echo "✓ Health check passed"
        break
    fi

    echo "  Waiting for service... attempt $i/30"
    sleep 10
done

if [ "$HEALTH" != "200" ]; then
    echo "ERROR: Health check failed after 5 minutes"
    exit 1
fi

# ===========================================
# Verify Agent Endpoints
# ===========================================
echo ""
echo "[7/8] Verifying agent endpoints..."

ENDPOINTS=(
    "api/v1/discovery/health:Config Discovery Agent"
    "api/v1/validation/health:Config Validation Agent"
    "api/v1/compatibility/health:Config Compatibility Agent"
    "api/v1/inspection/health:Config Inspection Agent"
)

ALL_HEALTHY=true
for ep in "${ENDPOINTS[@]}"; do
    path="${ep%%:*}"
    name="${ep##*:}"

    RESP=$(curl -s -o /dev/null -w "%{http_code}" \
        -H "Authorization: Bearer $TOKEN" \
        "$SERVICE_URL/$path" 2>/dev/null || echo "000")

    if [ "$RESP" = "200" ]; then
        echo "  ✓ $name is healthy"
    else
        echo "  ✗ $name returned $RESP"
        ALL_HEALTHY=false
    fi
done

if [ "$ALL_HEALTHY" = "false" ]; then
    echo "WARNING: Some agent endpoints are not healthy"
fi

# ===========================================
# Summary
# ===========================================
echo ""
echo "[8/8] Deployment Summary"
echo "=============================================="
echo "Service:     $SERVICE_NAME"
echo "Version:     $SERVICE_VERSION"
echo "URL:         $SERVICE_URL"
echo "Environment: $PLATFORM_ENV"
echo "Region:      $REGION"
echo ""
echo "Agent Endpoints:"
echo "  - $SERVICE_URL/api/v1/discovery"
echo "  - $SERVICE_URL/api/v1/validation"
echo "  - $SERVICE_URL/api/v1/compatibility"
echo "  - $SERVICE_URL/api/v1/inspection"
echo ""
echo "Health Check: $SERVICE_URL/health"
echo "Metrics:      $SERVICE_URL/metrics"
echo "=============================================="
echo ""
echo "✓ DEPLOYMENT COMPLETE"
echo ""
echo "Next steps:"
echo "  1. Update platform registry with service URL"
echo "  2. Verify CLI connectivity: agentics config validate --help"
echo "  3. Run smoke tests: ./verify-deployment.sh"
