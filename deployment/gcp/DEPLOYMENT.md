# LLM-Config-Manager Final Production Deployment

## Service Topology

### Unified Service Name
```
llm-config-manager
```

### Agent Endpoints (All Exposed by ONE Service)

| Agent | Endpoint | Purpose |
|-------|----------|---------|
| **Config Discovery Agent** | `/api/v1/discovery/*` | Discover and enumerate configurations |
| **Config Validation Agent** | `/api/v1/validation/*` | Validate configs against schemas |
| **Config Compatibility Agent** | `/api/v1/compatibility/*` | Cross-agent/service compatibility |
| **Config Inspection Agent** | `/api/v1/inspection/*` | Deep inspection, lineage, ownership |

### Confirmations

- ✅ **No agent is deployed as a standalone service**
- ✅ **Shared runtime** - All agents run in the same container
- ✅ **Shared configuration** - Single ConfigMap/Secret set
- ✅ **Shared telemetry stack** - Unified metrics and tracing

---

## Environment Configuration

### Required Environment Variables

| Variable | Description | Source |
|----------|-------------|--------|
| `RUVECTOR_SERVICE_URL` | ruvector-service endpoint | Secret Manager |
| `RUVECTOR_API_KEY` | ruvector-service auth key | Secret Manager |
| `PLATFORM_ENV` | Environment (dev/staging/prod) | ConfigMap |
| `TELEMETRY_ENDPOINT` | LLM-Observatory endpoint | ConfigMap |
| `SERVICE_NAME` | Service identifier | Static: `llm-config-manager` |
| `SERVICE_VERSION` | Deployed version | ConfigMap |
| `CONTRACTS_SCHEMA_VERSION` | agentics-contracts version | ConfigMap |

### Guarantees

- ✅ **No hardcoded service names or URLs**
- ✅ **No embedded credentials**
- ✅ **All dependencies resolve via environment variables or Secret Manager**

---

## Google SQL / Configuration Memory Wiring

### Confirmations

- ✅ **LLM-Config-Manager does NOT connect directly to Google SQL**
- ✅ **ALL DecisionEvents written via ruvector-service**
- ✅ **Schema compatibility with agentics-contracts validated at build time**
- ✅ **Append-only persistence behavior** (no updates, no deletes)
- ✅ **Idempotent writes** (same input → same DecisionEvent)
- ✅ **Retry safety** (duplicate writes are safe)

### Data Flow

```
┌─────────────────────┐     ┌──────────────────────┐     ┌─────────────┐
│ LLM-Config-Manager  │────▶│   ruvector-service   │────▶│ Google SQL  │
│  (Config Agents)    │     │   (HTTP Client)      │     │ (Postgres)  │
└─────────────────────┘     └──────────────────────┘     └─────────────┘
        │                            │
        │ DecisionEvents             │ Persists to DB
        │ (JSON over HTTPS)          │ (SQL)
        ▼                            ▼
   Read-Only Analysis          Append-Only Storage
```

---

## Cloud Build & Deployment

### Deployment Commands

```bash
# Set required environment variables
export PROJECT_ID="your-gcp-project"
export REGION="us-central1"
export SERVICE_VERSION="1.0.0"
export PLATFORM_ENV="production"

# Option 1: Full deployment script
./deployment/gcp/deploy.sh

# Option 2: Manual Cloud Build
gcloud builds submit \
    --project=$PROJECT_ID \
    --config=deployment/gcp/cloudbuild.yaml \
    --substitutions="_SERVICE_VERSION=$SERVICE_VERSION,_PLATFORM_ENV=$PLATFORM_ENV"

# Option 3: Direct Cloud Run deploy (if image exists)
gcloud run deploy llm-config-manager \
    --project=$PROJECT_ID \
    --region=$REGION \
    --image=gcr.io/$PROJECT_ID/llm-config-manager:$SERVICE_VERSION \
    --platform=managed \
    --ingress=internal \
    --no-allow-unauthenticated \
    --service-account=llm-config-manager-sa@$PROJECT_ID.iam.gserviceaccount.com
```

### IAM Service Account (Least Privilege)

| Role | Purpose |
|------|---------|
| `roles/run.invoker` | Internal service-to-service calls |
| `roles/secretmanager.secretAccessor` | Access secrets |
| `roles/logging.logWriter` | Write logs |
| `roles/monitoring.metricWriter` | Write metrics |
| `roles/cloudtrace.agent` | Distributed tracing |

**NOT Granted:**
- `roles/cloudsql.client` - No direct DB access
- `roles/storage.objectAdmin` - No file storage
- `roles/iam.serviceAccountAdmin` - No IAM changes

### Networking

- **Ingress**: Internal only (`--ingress=internal`)
- **Authentication**: Required (`--no-allow-unauthenticated`)
- **VPC**: Serverless VPC connector (optional, for private ruvector-service)

---

## CLI Activation Verification

### Available Commands

| Command | Agent | Example |
|---------|-------|---------|
| `agentics config discover` | Discovery | `agentics config discover --namespace myapp` |
| `agentics config validate` | Validation | `agentics config validate --file config.yaml` |
| `agentics config inspect` | Inspection | `agentics config inspect --namespace myapp --key db.host` |
| `agentics config compatibility` | Compatibility | `agentics config compatibility --configs a.yaml,b.yaml` |

### Example Invocations

```bash
# Discover all configs in namespace
agentics config discover --namespace myapp --env production --format json

# Validate configuration file
agentics config validate --file ./config/production.yaml --strict

# Inspect specific key
agentics config inspect --namespace myapp --key database.host --lineage

# Check compatibility
agentics config compatibility --namespace myapp --services api,worker
```

### Expected Success Output

```json
{
  "agent_id": "config-manager.validation.v1",
  "agent_version": "1.0.0",
  "decision_type": "config_validation_result",
  "inputs_hash": "sha256:abc123...",
  "outputs": {
    "valid": true,
    "findings": [],
    "rules_evaluated": 156
  },
  "confidence": 0.98,
  "constraints_applied": ["schema:v1.0.0", "env:production"],
  "execution_ref": "exec-xyz789",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

---

## Platform & Core Integration

### Integration Points (Read-Only)

| Consumer | Access | Purpose |
|----------|--------|---------|
| LLM-Orchestrator | Read | Planning reference |
| LLM-Policy-Engine | Read | Policy context |
| Governance/Audit | Consume | DecisionEvent audit |
| Analytics-Hub | Aggregate | Statistical analysis |
| Core Bundles | Read | Configuration artifacts |

### LLM-Config-Manager MUST NOT Invoke

- ❌ Runtime execution paths
- ❌ Enforcement layers (Policy-Engine/Shield)
- ❌ Optimization agents (Auto-Optimizer)
- ❌ Analytics pipelines (Analytics-Hub)
- ❌ Incident workflows

### No Core Bundle Rewiring

Config-Manager operates outside critical path. Core bundles consume artifacts without modification.

---

## Post-Deploy Verification Checklist

```bash
# Run automated verification
./deployment/gcp/verify-deployment.sh
```

### Manual Checklist

- [ ] LLM-Config-Manager service is live (`/health` returns 200)
- [ ] All 4 agent endpoints respond (`/api/v1/{discovery,validation,compatibility,inspection}/health`)
- [ ] Configuration validation is deterministic (same input → same output)
- [ ] Compatibility checks behave consistently across environments
- [ ] Inspection outputs are complete and schema-valid
- [ ] DecisionEvents appear in ruvector-service
- [ ] Telemetry appears in LLM-Observatory
- [ ] CLI config commands function end-to-end
- [ ] No direct SQL access from service account
- [ ] No agent bypasses agentics-contracts schemas

---

## Failure Modes & Rollback

### Common Deployment Failures

| Failure | Detection | Resolution |
|---------|-----------|------------|
| Missing schemas | Validation errors, 500s | Verify agentics-contracts version |
| Invalid config | Service won't start | Check ConfigMap/Secrets |
| Env mismatch | Wrong ruvector URL | Verify PLATFORM_ENV matches secrets |
| Auth failure | 401/403 from ruvector | Check RUVECTOR_API_KEY |
| Timeout | Slow responses | Check ruvector-service health |

### Detection Signals

- Health endpoint returns non-200
- DecisionEvents missing `agent_id` or `timestamp`
- Validation produces different results for same input
- Metrics show elevated error rates

### Rollback Procedure

```bash
# List previous revisions
gcloud run revisions list \
    --service=llm-config-manager \
    --region=$REGION \
    --project=$PROJECT_ID

# Rollback to previous revision
gcloud run services update-traffic llm-config-manager \
    --region=$REGION \
    --project=$PROJECT_ID \
    --to-revisions=llm-config-manager-PREVIOUS:100

# Verify rollback
curl -H "Authorization: Bearer $(gcloud auth print-identity-token)" \
    https://llm-config-manager-xxx.run.app/health
```

### Safe Redeploy Strategy

1. **Blue-Green**: Deploy new revision with 0% traffic
2. **Verify**: Run verification script against new revision
3. **Gradual**: Shift 10% → 50% → 100% traffic
4. **Monitor**: Watch error rates for 15 minutes
5. **Rollback**: If issues, shift traffic back immediately

```bash
# Deploy without traffic
gcloud run deploy llm-config-manager \
    --no-traffic \
    --tag=canary \
    ...

# Test canary
curl -H "Authorization: Bearer $TOKEN" \
    https://canary---llm-config-manager-xxx.run.app/health

# Shift traffic gradually
gcloud run services update-traffic llm-config-manager \
    --to-tags=canary=10

# Full rollout
gcloud run services update-traffic llm-config-manager \
    --to-latest
```

---

## Final Confirmation

✅ **This deployment produces an OPERATIONAL LLM-CONFIG-MANAGER service**

- Unified service with all 4 agents
- Read-only configuration intelligence
- No direct database access
- All persistence via ruvector-service
- Stateless, deterministic execution
- Full CLI integration
- Complete observability
