# LLM-Config-Manager Architecture Overview

## System Context Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        LLM DevOps Ecosystem                              │
│                                                                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                   │
│  │ Intelligence │  │  Security    │  │ Automation   │                   │
│  │    Core      │  │    Core      │  │    Core      │                   │
│  ├──────────────┤  ├──────────────┤  ├──────────────┤                   │
│  │Observatory   │  │Security-Guard│  │Auto-Optimizer│                   │
│  │Inference-Eng │  │              │  │              │                   │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘                   │
│         │                 │                 │                            │
│         │                 │                 │                            │
│  ┌──────┴─────────────────┴─────────────────┴───────┐                   │
│  │                                                    │                   │
│  │         LLM-Config-Manager (Central Hub)          │                   │
│  │                                                    │                   │
│  │  ┌──────────────┐  ┌──────────────┐              │                   │
│  │  │Configuration │  │   Secrets    │              │                   │
│  │  │   Storage    │  │  Management  │              │                   │
│  │  └──────────────┘  └──────────────┘              │                   │
│  │                                                    │                   │
│  │  ┌──────────────┐  ┌──────────────┐              │                   │
│  │  │   Version    │  │Multi-Tenant  │              │                   │
│  │  │   Control    │  │  Isolation   │              │                   │
│  │  └──────────────┘  └──────────────┘              │                   │
│  │                                                    │                   │
│  └──────┬─────────────────┬─────────────────┬────────┘                   │
│         │                 │                 │                            │
│         │                 │                 │                            │
│  ┌──────┴───────┐  ┌──────┴───────┐  ┌─────┴────────┐                  │
│  │  Governance  │  │     Data     │  │  Ecosystem   │                   │
│  │    Core      │  │    Core      │  │    Core      │                   │
│  ├──────────────┤  ├──────────────┤  ├──────────────┤                   │
│  │ Gov-Dashboard│  │Data-Pipeline │  │  Edge-Agent  │                   │
│  └──────────────┘  │Prompt-Registry│ └──────────────┘                   │
│                    └──────────────┘                                      │
│                                                                           │
│  ┌──────────────┐  ┌──────────────┐                                     │
│  │  Research    │  │  Interface   │                                     │
│  │    Core      │  │    Core      │                                     │
│  ├──────────────┤  ├──────────────┤                                     │
│  │   (Future)   │  │ API-Gateway  │                                     │
│  └──────────────┘  └──────────────┘                                     │
└─────────────────────────────────────────────────────────────────────────┘
```

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Client Layer                                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐            │
│  │ Rust SDK │  │Python SDK│  │  TS SDK  │  │   CLI    │            │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘            │
└───────┼─────────────┼─────────────┼─────────────┼──────────────────┘
        │             │             │             │
        │             │             │             │
┌───────┴─────────────┴─────────────┴─────────────┴──────────────────┐
│                         API Gateway Layer                            │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                      Load Balancer                            │  │
│  │                     (with mTLS/TLS)                           │  │
│  └─────────────────┬──────────────────┬─────────────────────────┘  │
│                    │                  │                             │
│         ┌──────────┴──────┐    ┌──────┴──────────┐                 │
│         │   REST API      │    │   gRPC API      │                 │
│         │ (OpenAPI 3.0)   │    │  (Protobuf)     │                 │
│         └──────────┬──────┘    └──────┬──────────┘                 │
└────────────────────┼────────────────────┼─────────────────────────┘
                     │                    │
┌────────────────────┴────────────────────┴─────────────────────────┐
│                    Core Service Layer                              │
│  ┌──────────────────────────────────────────────────────────────┐ │
│  │                  Authentication & Authorization               │ │
│  │      (mTLS, OAuth2/OIDC, API Keys, JWT, RBAC + OPA)          │ │
│  └──────────────────────────────────────────────────────────────┘ │
│                                                                    │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐               │
│  │  Config     │  │  Secrets    │  │  Version    │               │
│  │  Service    │  │  Service    │  │  Control    │               │
│  │             │  │             │  │  Service    │               │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘               │
│         │                │                │                        │
│  ┌──────┴──────┐  ┌──────┴──────┐  ┌──────┴──────┐               │
│  │  Validator  │  │  Rotation   │  │  Audit      │               │
│  │  Engine     │  │  Engine     │  │  Logger     │               │
│  └─────────────┘  └─────────────┘  └─────────────┘               │
│                                                                    │
└────────────────────────────────┬───────────────────────────────────┘
                                 │
┌────────────────────────────────┴───────────────────────────────────┐
│                      Cache & Storage Layer                          │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                   Distributed Cache (Redis/Memcached)         │  │
│  │                  TTL-based invalidation + Push updates        │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                     │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                │
│  │   Primary   │  │   Replica   │  │   Replica   │                │
│  │   Storage   │  │  (Region A) │  │  (Region B) │                │
│  │   (etcd/    │  │             │  │             │                │
│  │   Consul)   │  │             │  │             │                │
│  └─────────────┘  └─────────────┘  └─────────────┘                │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
                                 │
┌────────────────────────────────┴───────────────────────────────────┐
│                   External Integrations Layer                       │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                │
│  │  HashiCorp  │  │     AWS     │  │    Azure    │                │
│  │    Vault    │  │   Secrets   │  │  Key Vault  │                │
│  │             │  │   Manager   │  │             │                │
│  └─────────────┘  └─────────────┘  └─────────────┘                │
│                                                                     │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                │
│  │     GCP     │  │     KMS     │  │ OpenTelemetry│               │
│  │   Secret    │  │  (AWS/Azure │  │  (to LLM-   │                │
│  │   Manager   │  │   /GCP)     │  │ Observatory)│                │
│  └─────────────┘  └─────────────┘  └─────────────┘                │
└─────────────────────────────────────────────────────────────────────┘
```

## Multi-Tenant Isolation Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                      Tenant A (Healthcare)                           │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  Namespace: healthcare-prod                                   │  │
│  │  Encryption Key: DEK-A (from KMS)                             │  │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐              │  │
│  │  │ Config DB  │  │ Secrets DB │  │  Audit Log │              │  │
│  │  └────────────┘  └────────────┘  └────────────┘              │  │
│  │  Quota: 50K keys, 1M API calls/day                           │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                      Tenant B (Finance)                              │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  Namespace: finance-prod                                      │  │
│  │  Encryption Key: DEK-B (from KMS)                             │  │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐              │  │
│  │  │ Config DB  │  │ Secrets DB │  │  Audit Log │              │  │
│  │  └────────────┘  └────────────┘  └────────────┘              │  │
│  │  Quota: 100K keys, 10M API calls/day                         │  │
│  │  Data Residency: EU only                                      │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                      Tenant C (SaaS)                                 │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  Parent Namespace: saas-platform                              │  │
│  │  ┌────────────────────────────────────────────────────────┐  │  │
│  │  │  Child Tenant C1 (Customer 1)                          │  │  │
│  │  │  Namespace: saas-platform/customer-1                   │  │  │
│  │  │  Inherits: Global configs + Parent configs             │  │  │
│  │  └────────────────────────────────────────────────────────┘  │  │
│  │  ┌────────────────────────────────────────────────────────┐  │  │
│  │  │  Child Tenant C2 (Customer 2)                          │  │  │
│  │  │  Namespace: saas-platform/customer-2                   │  │  │
│  │  │  Inherits: Global configs + Parent configs             │  │  │
│  │  └────────────────────────────────────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

## Configuration Resolution Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│  Client Request: Get config "llm.openai.api_key" for               │
│  Tenant: healthcare, Environment: production, Service: inference    │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Step 1: Authentication & Authorization                             │
│  - Validate mTLS certificate                                        │
│  - Check RBAC permissions                                           │
│  - Verify tenant access                                             │
│  Result: ✓ Authorized                                               │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Step 2: Cache Lookup                                               │
│  Key: healthcare:production:inference:llm.openai.api_key            │
│  Result: MISS (not in cache)                                        │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Step 3: Configuration Resolution (Layered Lookup)                  │
│                                                                      │
│  Layer 1 (Global): llm.openai.api_key = null                        │
│  Layer 2 (Tenant): healthcare:llm.openai.api_key = null             │
│  Layer 3 (Environment): healthcare:production:llm.openai.api_key    │
│           = "{{secret:openai_key_prod}}"                            │
│  Layer 4 (Service): healthcare:production:inference:llm.openai...   │
│           = (override not found, use Layer 3)                       │
│                                                                      │
│  Final Value: "{{secret:openai_key_prod}}"                          │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Step 4: Variable Substitution                                      │
│  Template: "{{secret:openai_key_prod}}"                             │
│  Lookup secret: healthcare:openai_key_prod                          │
│  Decrypt with DEK-healthcare                                        │
│  Result: "sk-proj-abc123..."                                        │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Step 5: Validation                                                 │
│  - Check schema (string, length > 20)                               │
│  - Check expiration (not expired)                                   │
│  - Check security policy (not blacklisted)                          │
│  Result: ✓ Valid                                                    │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Step 6: Cache Update                                               │
│  Store in cache with TTL=300s                                       │
│  Subscribe to updates for push invalidation                         │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Step 7: Audit Logging                                              │
│  Log: config.read, tenant=healthcare, key=llm.openai.api_key,       │
│       user=inference-service, timestamp=2025-11-21T05:20:00Z        │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Response: "sk-proj-abc123..."                                      │
│  Latency: 45ms (cache miss, includes secret decryption)             │
│  Next request: ~2ms (from cache)                                    │
└─────────────────────────────────────────────────────────────────────┘
```

## Secret Rotation Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│  Scheduled Rotation: healthcare:openai_key_prod                     │
│  Schedule: Every 90 days, Next due: 2025-11-21T06:00:00Z            │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  T-15 minutes: Pre-Rotation Notification                            │
│  Event: secret.rotation.starting                                    │
│  Notify: All services using this secret                             │
│  Message: "Secret will rotate at 06:00, prepare for dual-key"       │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  T-0: Generate New Secret                                           │
│  1. Request new API key from OpenAI                                 │
│  2. Validate new key (test API call)                                │
│  3. Encrypt with DEK-healthcare                                     │
│  Result: New key "sk-proj-xyz789..." validated ✓                    │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  T+0: Dual-Key Phase Begins (7 day grace period)                    │
│  Version 1 (old): "sk-proj-abc123..." (valid until T+7 days)        │
│  Version 2 (new): "sk-proj-xyz789..." (preferred)                   │
│  Event: secret.rotated                                              │
│  Cache invalidation: Push update to all clients                     │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  T+1 to T+7 days: Monitoring Phase                                  │
│  - Monitor service health metrics                                   │
│  - Track usage of old vs new key                                    │
│  - Alert if old key usage doesn't decline                           │
│  - Ready to rollback if errors spike                                │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  T+7 days: Old Key Revocation                                       │
│  1. Verify no services using old key (usage = 0)                    │
│  2. Revoke old key at OpenAI                                        │
│  3. Delete old key version from storage                             │
│  4. Audit log: secret.revoked                                       │
│  Event: secret.rotation.completed                                   │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Rotation Complete                                                  │
│  Next rotation scheduled: T+90 days (2026-02-19T06:00:00Z)          │
│  Result: ✓ Zero-downtime rotation successful                        │
└─────────────────────────────────────────────────────────────────────┘
```

## Security Layers (Defense in Depth)

```
┌─────────────────────────────────────────────────────────────────────┐
│  Layer 1: Network Security                                          │
│  - VPC/VNET isolation                                               │
│  - Network policies (deny-all default)                              │
│  - DDoS protection                                                  │
└─────────────────────────────────────────────────────────────────────┘
                           │
┌─────────────────────────────────────────────────────────────────────┐
│  Layer 2: Transport Security                                        │
│  - TLS 1.3 for all connections                                      │
│  - mTLS for service-to-service                                      │
│  - Certificate pinning (optional)                                   │
└─────────────────────────────────────────────────────────────────────┘
                           │
┌─────────────────────────────────────────────────────────────────────┐
│  Layer 3: Authentication                                            │
│  - mTLS client certificates (services)                              │
│  - OAuth2/OIDC (human users)                                        │
│  - API keys with IP allowlisting                                    │
│  - JWT service tokens (short-lived)                                 │
└─────────────────────────────────────────────────────────────────────┘
                           │
┌─────────────────────────────────────────────────────────────────────┐
│  Layer 4: Authorization                                             │
│  - RBAC with predefined roles                                       │
│  - ABAC with Open Policy Agent                                      │
│  - Least privilege enforcement                                      │
│  - Scope-based permissions                                          │
└─────────────────────────────────────────────────────────────────────┘
                           │
┌─────────────────────────────────────────────────────────────────────┐
│  Layer 5: Data Encryption                                           │
│  - AES-256-GCM encryption at rest                                   │
│  - Field-level encryption for secrets                               │
│  - Per-tenant encryption keys (DEKs)                                │
│  - Envelope encryption with external KMS                            │
└─────────────────────────────────────────────────────────────────────┘
                           │
┌─────────────────────────────────────────────────────────────────────┐
│  Layer 6: Application Security                                      │
│  - Input validation and sanitization                                │
│  - SQL injection prevention (parameterized queries)                 │
│  - XSS protection (output encoding)                                 │
│  - CSRF protection (for web interfaces)                             │
└─────────────────────────────────────────────────────────────────────┘
                           │
┌─────────────────────────────────────────────────────────────────────┐
│  Layer 7: Audit & Monitoring                                        │
│  - Comprehensive audit logging                                      │
│  - Real-time security monitoring                                    │
│  - Anomaly detection                                                │
│  - Incident response automation                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow: Edge Agent Sync

```
┌─────────────────────────────────────────────────────────────────────┐
│  Edge Agent (IoT Device, Limited Bandwidth)                         │
│  Location: Factory floor, intermittent connectivity                 │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           │ 1. Connect (mTLS with device cert)
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  LLM-Config-Manager (Cloud)                                         │
│  Endpoint: /api/v1/edge/sync                                        │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           │ 2. Send current version hash
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Version Comparison                                                 │
│  Edge Version: v42 (hash: abc123...)                                │
│  Cloud Version: v45 (hash: def456...)                               │
│  Delta: 3 versions behind                                           │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           │ 3. Calculate delta
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Delta Calculation                                                  │
│  Changes from v42 to v45:                                           │
│  - Added: edge.sensor.config (12 KB)                                │
│  - Modified: llm.model.endpoint (0.5 KB)                            │
│  - Deleted: edge.deprecated.setting                                 │
│  Total delta size: 12.5 KB (vs 450 KB full config)                  │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           │ 4. Compress and send delta
                           │    (gzip compression)
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Edge Agent Receives Delta                                          │
│  Compressed size: 3.2 KB                                            │
│  Apply delta to local cache                                         │
│  Verify checksum                                                    │
│  Update version to v45                                              │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           │ 5. Acknowledge receipt
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Cloud Audit Log                                                    │
│  Event: edge.sync.completed                                         │
│  Device: factory-floor-sensor-42                                    │
│  Old version: v42, New version: v45                                 │
│  Bandwidth used: 3.2 KB (97% reduction vs full sync)                │
└─────────────────────────────────────────────────────────────────────┘
                           │
                           │ 6. Edge operates with updated config
                           │    (offline until next sync)
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Edge Agent Local Cache                                             │
│  Version: v45 (up-to-date)                                          │
│  Next sync: 24 hours or on reconnect                                │
│  Offline capability: ✓ Enabled                                      │
└─────────────────────────────────────────────────────────────────────┘
```

## Technology Stack

### Core Technologies
- **Language**: Rust (stable channel)
- **HTTP Framework**: axum or actix-web
- **gRPC**: tonic with Protocol Buffers
- **Storage**: etcd or Consul (distributed), sled or redb (embedded)
- **Cache**: Redis or Memcached
- **Encryption**: ring, rustls, age
- **Authentication**: oauth2, jsonwebtoken, openssl

### Infrastructure
- **Container**: Docker (OCI-compliant)
- **Orchestration**: Kubernetes
- **Service Mesh**: Istio or Linkerd (optional)
- **Certificate Management**: cert-manager
- **Monitoring**: Prometheus + Grafana
- **Tracing**: Jaeger or Tempo (OpenTelemetry)
- **Logging**: Loki or Elasticsearch

### External Services
- **Secret Stores**: HashiCorp Vault, AWS Secrets Manager, Azure Key Vault, GCP Secret Manager
- **KMS**: AWS KMS, Azure Key Vault, GCP Cloud KMS
- **Identity**: Okta, Auth0, Azure AD, Google Workspace
- **Observability**: LLM-Observatory (internal)

## Performance Characteristics

### Read Performance
- **Cache Hit (Hot Path)**: 2-5ms (in-memory)
- **Cache Miss (Cold Path)**: 40-100ms (network + decryption)
- **Batch Read (100 keys)**: 15-30ms (pipelined)

### Write Performance
- **Single Write**: 20-50ms (replicated, synced)
- **Batch Write (100 keys)**: 100-200ms (transaction)
- **Secret Rotation**: 200-500ms (validation + encryption)

### Scalability Metrics
- **Throughput**: 100K reads/sec, 5K writes/sec per instance
- **Concurrent Connections**: 10K+ per instance
- **Storage**: 1M+ keys, 100GB+ per deployment
- **Tenants**: 1000+ with full isolation

### Resource Usage
- **CPU**: 1-2 cores under normal load
- **Memory**: 1-2GB standard, 512MB edge
- **Network**: 50-100 Mbps per instance
- **Disk I/O**: 1000+ IOPS for writes

---

**Document Version**: 1.0.0
**Last Updated**: 2025-11-21
**Related Documents**: SPECIFICATION.json, SPECIFICATION_SUMMARY.md
