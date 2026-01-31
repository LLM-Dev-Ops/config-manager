# LLM-Config-Manager: Architecture Summary

**SPARC Phase:** Architecture
**Version:** 1.0.0
**Date:** 2025-11-21
**Status:** Complete

---

## Executive Summary

The LLM-Config-Manager architecture implements a **centralized configuration service with distributed caching**, designed for high availability, security, and performance in the LLM DevOps ecosystem. The system supports 20+ modules across 8 functional cores with strict multi-tenant isolation, comprehensive security, and flexible deployment options.

---

## Core Architectural Decisions

### 1. Technology Stack

| Component | Technology | Rationale |
|-----------|-----------|-----------|
| **Language** | Rust | Memory safety, performance, type safety |
| **REST API** | Axum (v0.7+) | Type-safe, ergonomic, Tower ecosystem |
| **gRPC** | Tonic (v0.11+) | Best Rust gRPC, 2.5x throughput vs REST |
| **Secrets Backend** | HashiCorp Vault | Industry standard, dynamic secrets, KMS integration |
| **Database** | PostgreSQL 14+ | ACID transactions, JSONB support, time-series partitioning |
| **Cache** | Redis Cluster | Sub-ms latency, pub/sub, high availability |
| **Authorization** | Open Policy Agent | Declarative policies, RBAC/ABAC support |
| **Observability** | OpenTelemetry | Distributed tracing, metrics, logs |

### 2. System Architecture Layers

```
┌─────────────────────────────────────────┐
│  Client Layer (CLI, SDKs, Dashboard)   │
├─────────────────────────────────────────┤
│  API Gateway (REST, gRPC, GraphQL, WS) │
├─────────────────────────────────────────┤
│  Service Layer (Config, Secret, Auth)  │
├─────────────────────────────────────────┤
│  Cache Layer (L1: Memory, L2: Redis)   │
├─────────────────────────────────────────┤
│  Storage (Vault, PostgreSQL, S3)       │
└─────────────────────────────────────────┘
```

### 3. Multi-Tenant Isolation Strategy

**Three-Tier Isolation:**

1. **Network Layer:** Tenant ID validation, per-tenant rate limiting
2. **Application Layer:** Logical isolation with separate caches and authorization contexts
3. **Data Layer:** Physical isolation with PostgreSQL schemas and per-tenant encryption keys

**Cryptographic Isolation:**
- Separate KEK (Key Encryption Key) per tenant in KMS
- Unique DEK (Data Encryption Key) per secret
- Envelope encryption pattern (DEK encrypts data, KEK encrypts DEK)
- Demonstrable compliance for auditors

### 4. Configuration Schema Hierarchy

```
/ (root)
├── global/                    # Lowest priority (defaults)
├── {tenant-id}/
│   ├── {environment}/        # dev, staging, prod, edge
│   │   ├── {module}/
│   │   │   ├── {component}/
│   │   │   │   ├── config.json
│   │   │   │   └── secrets.vault
│   │   │   └── shared/
│   │   └── common/
│   └── templates/
└── system/                    # Highest priority
```

**Resolution Chain:** `global → dev → staging → prod` (highest priority wins)

### 5. Data Models

**Core Entities:**
- **Configuration:** Versioned key-value with namespace, environment, tenant isolation
- **Secret:** Encrypted sensitive data with rotation policies
- **Namespace:** Hierarchical organization with quotas and permissions
- **Tenant:** Multi-tenant boundary with isolation mode and quotas
- **AuditLog:** Immutable audit trail with cryptographic signatures

**Version Control:**
- Git-style versioning with commit messages
- Full diff tracking (JSON Patch RFC 6902)
- Point-in-time rollback capability
- Minimum 90 days retention

### 6. Security Architecture

**Encryption:**
- **At Rest:** AES-256-GCM with envelope encryption
- **In Transit:** TLS 1.3 with mTLS for service-to-service
- **Field-Level:** Per-secret encryption with tenant-specific keys

**Authentication:**
- mTLS client certificates (services)
- OAuth2/OIDC (human users)
- API keys (legacy integrations)
- JWT service account tokens

**Authorization:**
- RBAC: 6 predefined roles (global-admin, tenant-admin, operator, developer, viewer, service-account)
- ABAC: Attribute-based policies via OPA
- Policy caching with 5-minute TTL
- Fail-closed (deny by default)

**Audit Logging:**
- All operations logged to PostgreSQL (partitioned by month)
- Structured JSON with CloudEvents envelope
- Cryptographic signatures for tamper evidence
- Real-time forwarding to LLM-Observatory

### 7. Caching Strategy

**Three-Layer Cache:**

| Layer | Storage | Latency | Capacity | TTL |
|-------|---------|---------|----------|-----|
| **L1** | In-memory LRU | < 1ms | 10K entries per instance | 5 min |
| **L2** | Redis Cluster | < 5ms | 100K entries shared | 5 min |
| **L3** | Vault/PostgreSQL | < 100ms | Unlimited | Source of truth |

**Cache Invalidation:**
- Push-based via Redis pub/sub
- TTL-based expiration as fallback
- Manual purge API for emergencies

**Target Metrics:**
- Combined L1+L2 hit rate: >90%
- Read latency p99: <10ms (cached), <100ms (uncached)

### 8. Integration Patterns

**Event-Driven Architecture:**
- CloudEvents v1.0 specification
- Kafka/Redis pub/sub for event distribution
- Event types: config.updated, secret.rotated, access.denied, etc.

**LLM DevOps Module Integrations:**

| Module | Integration Type | Protocol |
|--------|------------------|----------|
| LLM-Observatory | Telemetry export | OpenTelemetry (OTLP) |
| LLM-Edge-Agent | Configuration consumer | gRPC with delta sync |
| LLM-Governance-Dashboard | UI data provider | REST/GraphQL |
| LLM-Auto-Optimizer | Config consumer/producer | gRPC bidirectional |
| LLM-Security-Guard | Policy enforcement | gRPC validation hooks |
| LLM-Inference-Engine | Configuration consumer | gRPC with local cache |
| LLM-Policy-Engine | Authorization decisions | gRPC with policy cache |

### 9. Deployment Models

**Four Deployment Modes:**

1. **CLI Tool**
   - Standalone binary (Linux, macOS, Windows)
   - Local cache with sled embedded database
   - OS keychain integration
   - Auto-update capability

2. **API Service**
   - Kubernetes deployment (3+ replicas)
   - Horizontal autoscaling (3-10 instances)
   - REST, gRPC, GraphQL, WebSocket APIs
   - Prometheus metrics, OpenTelemetry traces

3. **Sidecar**
   - Lightweight container (<256MB RAM)
   - Init container for bootstrap
   - Watch mode for continuous sync
   - Signal-based application reload

4. **Edge**
   - Optimized binary (<50MB)
   - Offline-first with local persistence
   - Delta synchronization for bandwidth efficiency
   - Conflict resolution for bi-directional sync

### 10. Performance Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| **Read Latency (cached)** | p99 < 10ms | Load testing |
| **Write Latency** | p99 < 50ms | Load testing |
| **API Throughput** | >5000 req/sec per instance | Load testing |
| **Cache Hit Rate** | >90% combined L1+L2 | Production metrics |
| **Availability** | 99.99% uptime | Monthly uptime monitoring |
| **Data Durability** | 99.999999999% (11 nines) | Cross-region replication |

### 11. LLM-Specific Features

**Model Endpoint Configuration:**
- Provider-agnostic schema (OpenAI, Anthropic, AWS, Azure, GCP)
- Failover chains with automatic routing
- Model parameters validation (temperature, top_p, max_tokens)
- Cost tracking metadata

**Prompt Template Management:**
- Versioned templates with semantic versioning
- Variable substitution with validation
- Template inheritance
- A/B testing support

**API Credentials:**
- Secure storage with rotation
- Provider-specific metadata
- Expiration and renewal tracking

### 12. Operational Features

**Disaster Recovery:**
- Automated hourly backups (30-day retention)
- Point-in-time recovery (5-minute RPO)
- Cross-region replication
- Recovery time objective: <15 minutes

**Secret Rotation:**
- Scheduled rotation (hourly, daily, weekly, monthly)
- Pre-rotation notifications (15 minutes before)
- Dual-secret overlap period (zero downtime)
- Automatic rollback on failure
- Emergency rotation trigger

**Observability:**
- Structured JSON logs with trace correlation
- Prometheus metrics (/metrics endpoint)
- OpenTelemetry distributed tracing
- Health checks (/health/live, /health/ready)

---

## Key Architectural Patterns

### 1. Envelope Encryption (Per-Tenant Keys)

```
Secret → Encrypt with DEK → Ciphertext
DEK → Encrypt with KEK (from KMS) → Encrypted DEK
Store: [Ciphertext + Encrypted DEK]

Benefits:
- Cryptographic tenant isolation
- Efficient key rotation (re-encrypt DEK, not data)
- Audit trail in KMS
```

### 2. Multi-Layer Caching

```
Request → L1 Cache (in-memory) → if miss →
       → L2 Cache (Redis) → if miss →
       → L3 Storage (Vault/PostgreSQL)

Invalidation: Redis pub/sub + TTL-based expiration
```

### 3. Schema-Based Multi-Tenancy

```
PostgreSQL:
├── tenant_550e8400 (schema)
│   ├── configurations
│   ├── config_versions
│   └── secrets
├── tenant_660f9511 (schema)
│   ├── configurations
│   └── ...

Benefits:
- Strong isolation without database overhead
- Simple backup/restore per tenant
- Row-level security as additional safeguard
```

### 4. Event-Driven Configuration Updates

```
Config Update → Publish Event (Kafka/Redis) →
              → Subscribers invalidate cache →
              → Fetch updated config on next read

Zero-downtime configuration changes
```

### 5. Hybrid REST/gRPC API

```
REST (Axum):
- Administration and dashboard
- Human-friendly JSON
- OpenAPI documentation

gRPC (Tonic):
- High-performance module integration
- Streaming subscriptions
- Binary protocol efficiency (2.5x throughput)
```

---

## Component Responsibilities

### Configuration Service
- CRUD operations for configuration entries
- Environment-based override resolution
- Version management and history
- Template instantiation

### Secret Management Service
- Secure storage with field-level encryption
- Secret rotation scheduling and execution
- Integration with Vault and cloud KMS
- Expiration and lifecycle management

### Authentication Service
- Multi-method authentication (mTLS, OAuth2, API keys, JWT)
- Token validation and renewal
- Session management
- External IdP integration

### Authorization Service
- Policy evaluation (RBAC/ABAC via OPA)
- Permission caching with invalidation
- Audit logging of access decisions
- Context-aware policies (time, IP, environment)

### Audit Logger
- Immutable append-only logs
- PostgreSQL time-series partitioning
- Cryptographic signatures
- Real-time event bus publishing

### Cache Manager
- L1: In-memory LRU cache
- L2: Redis distributed cache
- Push-based invalidation via pub/sub
- Configurable TTL per namespace

### Validation Service
- JSON Schema validation
- Custom validation rules
- Cross-field dependencies
- External validation hooks

---

## Data Flow Examples

### 1. Configuration Read (Happy Path)

```
1. Client → gRPC GetConfig(namespace, key, env)
2. API Gateway → Authentication → Authorization
3. Config Service → L1 Cache → HIT → Return value (1ms)
```

### 2. Configuration Read (Cache Miss)

```
1. Client → gRPC GetConfig(namespace, key, env)
2. API Gateway → Authentication → Authorization
3. Config Service → L1 Cache → MISS
4. Config Service → L2 Cache (Redis) → HIT → Return value (5ms)
5. Config Service → Populate L1 Cache
```

### 3. Configuration Read (Full Miss)

```
1. Client → gRPC GetConfig(namespace, key, env)
2. API Gateway → Authentication → Authorization
3. Config Service → L1 Cache → MISS
4. Config Service → L2 Cache → MISS
5. Config Service → PostgreSQL → Fetch value (50ms)
6. Config Service → Populate L2 and L1 caches
7. Config Service → Audit log access
8. Return value
```

### 4. Configuration Write

```
1. Client → gRPC SetConfig(namespace, key, value)
2. API Gateway → Authentication → Authorization
3. Validation Service → Schema validation
4. Config Service → Create new version in PostgreSQL
5. Config Service → Invalidate L1 and L2 caches (pub/sub)
6. Config Service → Publish event (Kafka: config.updated)
7. Audit Logger → Log change with diff
8. Return new version
```

### 5. Secret Rotation

```
1. Rotation Scheduler → Trigger rotation
2. Secret Service → Generate new secret
3. Secret Service → Notify dependent services (15 min before)
4. Secret Service → Store new version in Vault
5. Secret Service → Keep old version valid (grace period)
6. Secret Service → Publish event (secret.rotated)
7. After grace period → Mark old version as deprecated
8. Audit Logger → Log rotation
```

---

## Database Schema Highlights

**PostgreSQL Tables:**
- `tenants` - Tenant metadata and quotas
- `namespaces` - Hierarchical organization
- `configurations` - Configuration entries
- `config_versions` - Version history with diffs
- `secrets` - Secret metadata (encrypted values in Vault)
- `audit_logs` - Partitioned by month for performance
- `schemas` - JSON Schema registry
- `rotation_schedules` - Automated rotation config
- `backups` - Backup metadata and status

**Partitioning Strategy:**
- Audit logs partitioned by month (automatic partition creation)
- Retention: 90 days hot, 1 year warm, 7 years cold (S3 Glacier)

**Indexing Strategy:**
- Tenant ID indexes on all tenant-scoped tables
- Composite indexes for common queries (namespace + key + environment)
- Time-series indexes on audit logs (timestamp DESC)
- GIN indexes for full-text search (namespace paths)

---

## API Endpoints Summary

### REST API (Axum)
```
POST   /api/v1/configs/{namespace}/{key}      # Create/update config
GET    /api/v1/configs/{namespace}/{key}      # Get config
DELETE /api/v1/configs/{namespace}/{key}      # Delete config
GET    /api/v1/configs/{namespace}            # List configs
GET    /api/v1/configs/{namespace}/history    # Version history

GET    /health/live                           # Liveness probe
GET    /health/ready                          # Readiness probe
GET    /metrics                               # Prometheus metrics
```

### gRPC API (Tonic)
```protobuf
service ConfigService {
  rpc GetConfig(GetConfigRequest) returns (GetConfigResponse);
  rpc SetConfig(SetConfigRequest) returns (SetConfigResponse);
  rpc DeleteConfig(DeleteConfigRequest) returns (DeleteConfigResponse);
  rpc ListConfigs(ListConfigsRequest) returns (stream ConfigEntry);
  rpc WatchConfig(WatchConfigRequest) returns (stream ConfigChange);
}

service SecretService {
  rpc GetSecret(GetSecretRequest) returns (GetSecretResponse);
  rpc RotateSecret(RotateSecretRequest) returns (RotateSecretResponse);
}
```

---

## Security Compliance Mapping

| Framework | Controls | Implementation |
|-----------|----------|----------------|
| **SOC 2 Type II** | Access controls, encryption, monitoring | RBAC, mTLS, audit logs |
| **GDPR** | Data protection, right to be forgotten | Per-tenant keys, deletion APIs |
| **HIPAA** | PHI protection, audit trails | Encryption, access logging, BAAs |
| **PCI-DSS** | Cardholder data security | Tokenization, vulnerability scans |
| **ISO 27001** | Information security management | Comprehensive security controls |

---

## Monitoring and Alerting

**Key Metrics:**
- `config_operations_total{operation, namespace, status}` - Counter
- `config_operation_duration_seconds{operation}` - Histogram
- `cache_hit_ratio{layer}` - Gauge
- `active_configurations{tenant_id, environment}` - Gauge
- `secret_rotation_status{namespace, key}` - Gauge
- `vault_latency_seconds{operation}` - Histogram
- `policy_evaluation_duration_seconds{result}` - Histogram

**Alerts:**
- Cache hit rate <80% for 10 minutes
- Secret rotation failure
- Authorization denied rate >1% for 5 minutes
- Vault connectivity issues
- Database connection pool exhaustion
- Quota exceeded for tenant

---

## Next Steps

This architecture is now ready for the **Refinement Phase** (SPARC R):

1. **Implementation Planning:**
   - Create detailed Rust module structure
   - Define API contracts (OpenAPI, Protobuf)
   - Set up CI/CD pipelines

2. **MVP Development (Sprints 1-4):**
   - Core configuration CRUD
   - Basic encryption
   - File-based storage
   - CLI tool
   - Configuration versioning

3. **Beta Development (Sprints 5-10):**
   - Vault integration
   - RBAC implementation
   - REST API service
   - Multi-tenant isolation
   - Performance optimization

4. **v1.0 Development (Sprints 11-16):**
   - Full feature set
   - Production deployment
   - Comprehensive testing
   - Documentation and training

---

**Document Status:** Complete and ready for implementation
**Next Document:** Pseudocode specification for core algorithms
**Related Documents:**
- ARCHITECTURE-PHASE.md (full details)
- SPECIFICATION.json (requirements)
- SPARC-ALIGNED-ROADMAP.md (delivery plan)
- RESEARCH_SUMMARY.md (technology research)
