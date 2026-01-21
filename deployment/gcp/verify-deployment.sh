#!/bin/bash
# =============================================================================
# LLM-CONFIG-MANAGER POST-DEPLOYMENT VERIFICATION
# =============================================================================

set -euo pipefail

# ===========================================
# Configuration
# ===========================================
PROJECT_ID="${PROJECT_ID:?PROJECT_ID environment variable required}"
REGION="${REGION:-us-central1}"
SERVICE_NAME="llm-config-manager"

echo "=============================================="
echo "LLM-CONFIG-MANAGER DEPLOYMENT VERIFICATION"
echo "=============================================="

# Get service URL
SERVICE_URL=$(gcloud run services describe "$SERVICE_NAME" \
    --project="$PROJECT_ID" \
    --region="$REGION" \
    --format='value(status.url)')

TOKEN=$(gcloud auth print-identity-token)

PASS=0
FAIL=0
WARN=0

check() {
    local name="$1"
    local result="$2"
    local expected="$3"

    if [ "$result" = "$expected" ]; then
        echo "  ✓ $name"
        ((PASS++))
    else
        echo "  ✗ $name (got: $result, expected: $expected)"
        ((FAIL++))
    fi
}

warn() {
    local name="$1"
    local message="$2"
    echo "  ⚠ $name: $message"
    ((WARN++))
}

# ===========================================
# 1. Service Availability
# ===========================================
echo ""
echo "[1/10] Checking service availability..."

HEALTH=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "Authorization: Bearer $TOKEN" \
    "$SERVICE_URL/health" 2>/dev/null || echo "000")
check "LLM-Config-Manager service is live" "$HEALTH" "200"

# ===========================================
# 2. Agent Endpoints Respond
# ===========================================
echo ""
echo "[2/10] Checking agent endpoints..."

for endpoint in discovery validation compatibility inspection; do
    RESP=$(curl -s -o /dev/null -w "%{http_code}" \
        -H "Authorization: Bearer $TOKEN" \
        "$SERVICE_URL/api/v1/$endpoint/health" 2>/dev/null || echo "000")
    check "$endpoint agent endpoint responds" "$RESP" "200"
done

# ===========================================
# 3. Configuration Validation is Deterministic
# ===========================================
echo ""
echo "[3/10] Checking validation determinism..."

TEST_CONFIG='{"database":{"host":"localhost","port":5432}}'

RESULT1=$(curl -s \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d "$TEST_CONFIG" \
    "$SERVICE_URL/api/v1/validation/validate" 2>/dev/null | md5sum | cut -d' ' -f1)

RESULT2=$(curl -s \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d "$TEST_CONFIG" \
    "$SERVICE_URL/api/v1/validation/validate" 2>/dev/null | md5sum | cut -d' ' -f1)

if [ "$RESULT1" = "$RESULT2" ]; then
    echo "  ✓ Validation is deterministic"
    ((PASS++))
else
    echo "  ✗ Validation is NOT deterministic"
    ((FAIL++))
fi

# ===========================================
# 4. Compatibility Checks Consistent
# ===========================================
echo ""
echo "[4/10] Checking compatibility consistency..."

COMPAT_REQ='{"configs":[{"name":"config1","data":{}},{"name":"config2","data":{}}]}'

COMPAT1=$(curl -s \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d "$COMPAT_REQ" \
    "$SERVICE_URL/api/v1/compatibility/check" 2>/dev/null | jq -r '.outputs.compatible // "error"')

COMPAT2=$(curl -s \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d "$COMPAT_REQ" \
    "$SERVICE_URL/api/v1/compatibility/check" 2>/dev/null | jq -r '.outputs.compatible // "error"')

if [ "$COMPAT1" = "$COMPAT2" ]; then
    echo "  ✓ Compatibility checks are consistent"
    ((PASS++))
else
    echo "  ✗ Compatibility checks are NOT consistent"
    ((FAIL++))
fi

# ===========================================
# 5. Inspection Outputs are Schema-Valid
# ===========================================
echo ""
echo "[5/10] Checking inspection output schema..."

INSPECT_RESP=$(curl -s \
    -H "Authorization: Bearer $TOKEN" \
    "$SERVICE_URL/api/v1/inspection/inspect?namespace=test&key=test" 2>/dev/null)

# Check required fields in DecisionEvent
HAS_AGENT_ID=$(echo "$INSPECT_RESP" | jq -r 'has("agent_id")')
HAS_DECISION_TYPE=$(echo "$INSPECT_RESP" | jq -r 'has("decision_type")')
HAS_OUTPUTS=$(echo "$INSPECT_RESP" | jq -r 'has("outputs")')
HAS_TIMESTAMP=$(echo "$INSPECT_RESP" | jq -r 'has("timestamp")')

check "DecisionEvent has agent_id" "$HAS_AGENT_ID" "true"
check "DecisionEvent has decision_type" "$HAS_DECISION_TYPE" "true"
check "DecisionEvent has outputs" "$HAS_OUTPUTS" "true"
check "DecisionEvent has timestamp" "$HAS_TIMESTAMP" "true"

# ===========================================
# 6. DecisionEvents in ruvector-service
# ===========================================
echo ""
echo "[6/10] Checking DecisionEvent persistence..."

# This check requires ruvector-service access - may need adjustment
RUVECTOR_URL="${RUVECTOR_SERVICE_URL:-https://ruvector.agentics.internal/v1}"

# Attempt to query recent events (will fail gracefully if no access)
EVENTS_CHECK=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "Authorization: Bearer $TOKEN" \
    "$RUVECTOR_URL/events?agent_id=config-manager&limit=1" 2>/dev/null || echo "skip")

if [ "$EVENTS_CHECK" = "200" ]; then
    echo "  ✓ DecisionEvents accessible in ruvector-service"
    ((PASS++))
elif [ "$EVENTS_CHECK" = "skip" ]; then
    warn "DecisionEvents persistence" "Cannot verify - ruvector-service not accessible from this context"
else
    warn "DecisionEvents persistence" "ruvector-service returned $EVENTS_CHECK"
fi

# ===========================================
# 7. Telemetry in LLM-Observatory
# ===========================================
echo ""
echo "[7/10] Checking telemetry emission..."

# Check metrics endpoint
METRICS=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "Authorization: Bearer $TOKEN" \
    "$SERVICE_URL/metrics" 2>/dev/null || echo "000")

check "Metrics endpoint available" "$METRICS" "200"

# ===========================================
# 8. CLI Config Commands
# ===========================================
echo ""
echo "[8/10] Checking CLI connectivity..."

# These checks assume agentics-cli is installed
if command -v agentics &>/dev/null; then
    DISCOVER_HELP=$(agentics config discover --help 2>&1 || echo "error")
    if [[ "$DISCOVER_HELP" != *"error"* ]]; then
        echo "  ✓ CLI discover command available"
        ((PASS++))
    else
        echo "  ✗ CLI discover command failed"
        ((FAIL++))
    fi

    VALIDATE_HELP=$(agentics config validate --help 2>&1 || echo "error")
    if [[ "$VALIDATE_HELP" != *"error"* ]]; then
        echo "  ✓ CLI validate command available"
        ((PASS++))
    else
        echo "  ✗ CLI validate command failed"
        ((FAIL++))
    fi
else
    warn "CLI commands" "agentics-cli not installed in this environment"
fi

# ===========================================
# 9. No Direct SQL Access
# ===========================================
echo ""
echo "[9/10] Verifying no direct SQL access..."

# Check that service account doesn't have SQL permissions
SA_EMAIL="${SERVICE_NAME}-sa@${PROJECT_ID}.iam.gserviceaccount.com"

SQL_ROLES=$(gcloud projects get-iam-policy "$PROJECT_ID" \
    --flatten="bindings[].members" \
    --filter="bindings.members:serviceAccount:$SA_EMAIL AND bindings.role:roles/cloudsql" \
    --format="value(bindings.role)" 2>/dev/null || echo "")

if [ -z "$SQL_ROLES" ]; then
    echo "  ✓ Service account has no Cloud SQL permissions"
    ((PASS++))
else
    echo "  ✗ Service account has Cloud SQL permissions: $SQL_ROLES"
    ((FAIL++))
fi

# ===========================================
# 10. agentics-contracts Compliance
# ===========================================
echo ""
echo "[10/10] Checking agentics-contracts compliance..."

# Verify schema version is declared
SCHEMA_VERSION=$(curl -s \
    -H "Authorization: Bearer $TOKEN" \
    "$SERVICE_URL/api/v1/validation/schema" 2>/dev/null | jq -r '.version // "unknown"')

if [ "$SCHEMA_VERSION" != "unknown" ] && [ "$SCHEMA_VERSION" != "null" ]; then
    echo "  ✓ Schema version declared: $SCHEMA_VERSION"
    ((PASS++))
else
    warn "Schema version" "Could not retrieve schema version"
fi

# ===========================================
# Summary
# ===========================================
echo ""
echo "=============================================="
echo "VERIFICATION SUMMARY"
echo "=============================================="
echo "  Passed:   $PASS"
echo "  Failed:   $FAIL"
echo "  Warnings: $WARN"
echo "=============================================="

if [ "$FAIL" -gt 0 ]; then
    echo ""
    echo "❌ VERIFICATION FAILED"
    echo "Please review failed checks before proceeding."
    exit 1
elif [ "$WARN" -gt 0 ]; then
    echo ""
    echo "⚠️  VERIFICATION PASSED WITH WARNINGS"
    echo "Review warnings and verify manually if needed."
    exit 0
else
    echo ""
    echo "✅ ALL VERIFICATIONS PASSED"
    exit 0
fi
