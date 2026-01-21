# LLM-Config-Manager CLI Activation & Commands

## Overview

All LLM-Config-Manager agents are callable via `agentics-cli`. The CLI configuration resolves the service URL dynamically from environment or platform registry.

## CLI Configuration

```bash
# Set service URL dynamically (no hardcoding)
export LLM_CONFIG_MANAGER_URL="${AGENTICS_PLATFORM_URL}/config-manager"

# Or use platform registry
agentics config set llm-config-manager.url auto
```

---

## Agent CLI Commands

### 1. Config Discovery Agent

**Purpose:** Discover and enumerate configuration across services and agents.

```bash
# Discover all configurations in a namespace
agentics config discover --namespace myapp

# Discover with environment filter
agentics config discover --namespace myapp --env production

# Discover with output format
agentics config discover --namespace myapp --format json

# Discover across all namespaces (admin only)
agentics config discover --all --format table
```

**Expected Success Output:**
```json
{
  "agent_id": "config-manager.discovery.v1",
  "decision_type": "config_discovery_result",
  "confidence": 0.95,
  "outputs": {
    "discovered_configs": [
      {
        "namespace": "myapp",
        "key": "database.host",
        "environment": "production",
        "source": "declared",
        "last_modified": "2024-01-15T10:30:00Z"
      }
    ],
    "total_count": 42
  },
  "execution_ref": "exec-abc123"
}
```

---

### 2. Config Validation Agent

**Purpose:** Validate configuration against schemas and environment constraints.

```bash
# Validate a configuration file
agentics config validate --file config.yaml

# Validate with specific schema
agentics config validate --file config.yaml --schema schemas/v1.json

# Validate for specific environment
agentics config validate --file config.yaml --env production

# Validate namespace configurations
agentics config validate --namespace myapp --env staging

# Validate with strict mode (fail on warnings)
agentics config validate --file config.yaml --strict
```

**Expected Success Output:**
```json
{
  "agent_id": "config-manager.validation.v1",
  "decision_type": "config_validation_result",
  "confidence": 0.98,
  "outputs": {
    "valid": true,
    "findings": [],
    "rules_evaluated": 156,
    "rules_passed": 156,
    "schema_version": "1.0.0"
  },
  "constraints_applied": [
    "schema:production-v1.0.0",
    "environment:production",
    "rules:required,type,bounds,enum"
  ],
  "execution_ref": "exec-def456"
}
```

**Validation Failure Output:**
```json
{
  "agent_id": "config-manager.validation.v1",
  "decision_type": "config_validation_result",
  "confidence": 0.92,
  "outputs": {
    "valid": false,
    "findings": [
      {
        "severity": "error",
        "rule_id": "required.database.host",
        "field_path": "database.host",
        "message": "Required field 'database.host' is missing",
        "suggestion": "Add database.host configuration"
      },
      {
        "severity": "warning",
        "rule_id": "deprecated.cache.legacy_mode",
        "field_path": "cache.legacy_mode",
        "message": "Field 'cache.legacy_mode' is deprecated since v2.0",
        "suggestion": "Use 'cache.mode' instead"
      }
    ],
    "rules_evaluated": 156,
    "rules_passed": 154
  },
  "execution_ref": "exec-ghi789"
}
```

---

### 3. Config Compatibility Agent

**Purpose:** Analyze cross-agent and cross-service compatibility.

```bash
# Check compatibility between two configs
agentics config compatibility --configs config1.yaml,config2.yaml

# Check namespace compatibility
agentics config compatibility --namespace myapp --services api,worker,scheduler

# Check environment migration compatibility
agentics config compatibility --namespace myapp --from staging --to production

# Check version upgrade compatibility
agentics config compatibility --namespace myapp --from v1.0 --to v2.0
```

**Expected Success Output:**
```json
{
  "agent_id": "config-manager.compatibility.v1",
  "decision_type": "config_compatibility_result",
  "confidence": 0.96,
  "outputs": {
    "compatible": true,
    "compatibility_score": 0.98,
    "issues": [],
    "recommendations": [
      {
        "type": "optimization",
        "message": "Consider aligning cache TTL values across services"
      }
    ]
  },
  "constraints_applied": [
    "scope:cross-service",
    "services:api,worker,scheduler"
  ],
  "execution_ref": "exec-jkl012"
}
```

---

### 4. Config Inspection Agent

**Purpose:** Deep inspection of configuration structure, lineage, and ownership.

```bash
# Inspect a configuration key
agentics config inspect --namespace myapp --key database.host

# Inspect with full lineage
agentics config inspect --namespace myapp --key database.host --lineage

# Inspect all keys in namespace
agentics config inspect --namespace myapp --all

# Inspect with ownership information
agentics config inspect --namespace myapp --key database.host --ownership

# Export inspection report
agentics config inspect --namespace myapp --format json --output report.json
```

**Expected Success Output:**
```json
{
  "agent_id": "config-manager.inspection.v1",
  "decision_type": "config_inspection_result",
  "confidence": 1.0,
  "outputs": {
    "key": "database.host",
    "namespace": "myapp",
    "current_value": "db.production.internal",
    "value_type": "string",
    "environment": "production",
    "lineage": {
      "created_at": "2023-06-01T00:00:00Z",
      "created_by": "platform-init",
      "last_modified": "2024-01-10T15:30:00Z",
      "modified_by": "user:admin@company.com",
      "version": 5,
      "change_history": [
        {"version": 4, "timestamp": "2024-01-05T10:00:00Z", "actor": "user:dev@company.com"},
        {"version": 5, "timestamp": "2024-01-10T15:30:00Z", "actor": "user:admin@company.com"}
      ]
    },
    "ownership": {
      "team": "platform",
      "service": "api",
      "classification": "internal"
    },
    "schema": {
      "type": "string",
      "format": "hostname",
      "required": true
    }
  },
  "execution_ref": "exec-mno345"
}
```

---

## CLI Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success - validation passed / operation complete |
| 1 | Validation failed - errors found |
| 2 | Validation warnings - no errors but warnings present |
| 10 | Configuration error - invalid CLI arguments |
| 11 | Service unavailable - cannot reach llm-config-manager |
| 12 | Authentication error - invalid credentials |
| 20 | Schema error - invalid or missing schema |
| 30 | Internal error - unexpected service error |

---

## CLI Configuration File

Location: `~/.agentics/config.yaml`

```yaml
# LLM-Config-Manager CLI Configuration
llm-config-manager:
  # Service URL (auto-resolved from platform registry by default)
  url: auto

  # Timeout settings
  timeout_seconds: 30

  # Default output format
  default_format: json

  # Enable verbose output
  verbose: false

  # Cache responses locally
  cache_enabled: true
  cache_ttl_seconds: 300
```

---

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `LLM_CONFIG_MANAGER_URL` | Service URL override | (from platform registry) |
| `AGENTICS_API_KEY` | API authentication key | (required) |
| `AGENTICS_PLATFORM_ENV` | Platform environment | `production` |
| `LLM_CONFIG_OUTPUT_FORMAT` | Default output format | `json` |
| `LLM_CONFIG_VERBOSE` | Enable verbose mode | `false` |

---

## No CLI Change Required for Agent Updates

The CLI dynamically resolves:
- Service URLs from platform registry
- Schema versions from service metadata
- Available commands from service capabilities

Agent updates do **NOT** require CLI redeployment.
