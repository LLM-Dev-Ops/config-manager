#!/bin/bash
# =============================================================================
# LLM-CONFIG-MANAGER IAM SETUP
# Least privilege service account configuration
# =============================================================================

set -euo pipefail

# Configuration
PROJECT_ID="${PROJECT_ID:?PROJECT_ID environment variable required}"
SERVICE_NAME="llm-config-manager"
SERVICE_ACCOUNT="${SERVICE_NAME}-sa"
REGION="${REGION:-us-central1}"

echo "Setting up IAM for LLM-Config-Manager..."
echo "Project: $PROJECT_ID"
echo "Service Account: $SERVICE_ACCOUNT"

# ===========================================
# Create Service Account
# ===========================================
echo "Creating service account..."
gcloud iam service-accounts create "$SERVICE_ACCOUNT" \
    --project="$PROJECT_ID" \
    --display-name="LLM Config Manager Service Account" \
    --description="Service account for LLM-Config-Manager with minimal permissions" \
    2>/dev/null || echo "Service account already exists"

SA_EMAIL="${SERVICE_ACCOUNT}@${PROJECT_ID}.iam.gserviceaccount.com"

# ===========================================
# Grant Minimal Permissions (Least Privilege)
# ===========================================
echo "Granting minimal permissions..."

# Cloud Run invoker (for internal service-to-service calls)
gcloud projects add-iam-policy-binding "$PROJECT_ID" \
    --member="serviceAccount:$SA_EMAIL" \
    --role="roles/run.invoker" \
    --condition=None

# Secret Manager accessor (for RUVECTOR_API_KEY, etc.)
gcloud projects add-iam-policy-binding "$PROJECT_ID" \
    --member="serviceAccount:$SA_EMAIL" \
    --role="roles/secretmanager.secretAccessor" \
    --condition=None

# Cloud Logging writer (for telemetry)
gcloud projects add-iam-policy-binding "$PROJECT_ID" \
    --member="serviceAccount:$SA_EMAIL" \
    --role="roles/logging.logWriter" \
    --condition=None

# Cloud Monitoring writer (for metrics)
gcloud projects add-iam-policy-binding "$PROJECT_ID" \
    --member="serviceAccount:$SA_EMAIL" \
    --role="roles/monitoring.metricWriter" \
    --condition=None

# Cloud Trace agent (for distributed tracing)
gcloud projects add-iam-policy-binding "$PROJECT_ID" \
    --member="serviceAccount:$SA_EMAIL" \
    --role="roles/cloudtrace.agent" \
    --condition=None

# ===========================================
# Create Secrets in Secret Manager
# ===========================================
echo "Creating secrets..."

# RUVECTOR_SERVICE_URL secret
echo -n "https://ruvector.agentics.internal/v1" | \
gcloud secrets create llm-config-ruvector-url \
    --project="$PROJECT_ID" \
    --data-file=- \
    2>/dev/null || \
gcloud secrets versions add llm-config-ruvector-url \
    --project="$PROJECT_ID" \
    --data-file=- <<< "https://ruvector.agentics.internal/v1"

# RUVECTOR_API_KEY secret (placeholder - replace in production)
echo "Creating RUVECTOR_API_KEY secret placeholder..."
echo -n "REPLACE_WITH_ACTUAL_API_KEY" | \
gcloud secrets create llm-config-ruvector-key \
    --project="$PROJECT_ID" \
    --data-file=- \
    2>/dev/null || echo "Secret already exists - update manually if needed"

# ===========================================
# Grant Secret Access
# ===========================================
echo "Granting secret access..."

for secret in llm-config-ruvector-url llm-config-ruvector-key; do
    gcloud secrets add-iam-policy-binding "$secret" \
        --project="$PROJECT_ID" \
        --member="serviceAccount:$SA_EMAIL" \
        --role="roles/secretmanager.secretAccessor"
done

# ===========================================
# Verify Setup
# ===========================================
echo ""
echo "=== IAM Setup Complete ==="
echo "Service Account: $SA_EMAIL"
echo ""
echo "Permissions granted:"
echo "  - roles/run.invoker (Cloud Run invocation)"
echo "  - roles/secretmanager.secretAccessor (Secret access)"
echo "  - roles/logging.logWriter (Logging)"
echo "  - roles/monitoring.metricWriter (Metrics)"
echo "  - roles/cloudtrace.agent (Tracing)"
echo ""
echo "Secrets created:"
echo "  - llm-config-ruvector-url"
echo "  - llm-config-ruvector-key (update with actual key)"
echo ""
echo "IMPORTANT: This service account does NOT have:"
echo "  - Database access (cloudsql.client)"
echo "  - Storage write access"
echo "  - IAM admin permissions"
echo "  - Any deployment/modification permissions"
echo ""
echo "All persistence MUST go through ruvector-service."
