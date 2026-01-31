# LLM-Config-Manager Architecture Diagrams

This document provides visual representations of the LLM-Config-Manager architecture.

## System Context Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         LLM DevOps Ecosystem                            │
│                                                                         │
│  ┌──────────────┐      ┌──────────────┐      ┌──────────────┐        │
│  │ LLM-Policy   │─────▶│ LLM-Config   │◀─────│ LLM-Auto     │        │
│  │ Engine       │ gRPC │ Manager      │ REST │ Optimizer    │        │
│  │ (RBAC/Policy)│      │              │      │              │        │
│  └──────────────┘      └──────┬───────┘      └──────────────┘        │
│                               │                                        │
│  ┌──────────────┐            │            ┌──────────────┐           │
│  │ LLM-         │◀───────────┼────────────│ LLM-         │           │
│  │ Governance   │ WebSocket  │            │ Observatory  │           │
│  │ Dashboard    │            │ Prometheus │              │           │
│  └──────────────┘            │            └──────────────┘           │
│                               │                                        │
│  ┌──────────────┐            │                                        │
│  │ LLM-Edge     │◀───────────┘                                        │
│  │ Agent        │ gRPC Stream                                         │
│  └──────────────┘                                                     │
└─────────────────────────────────────────────────────────────────────────┘
                               │
              ┌────────────────┼────────────────┐
              ▼                ▼                ▼
      ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
      │ HashiCorp   │  │ AWS KMS     │  │ GCP Cloud   │
      │ Vault       │  │             │  │ KMS         │
      └─────────────┘  └─────────────┘  └─────────────┘
```

## Deployment Architecture Comparison

```
┌────────────────────────────────────────────────────────────────────────┐
│                    CLI Management Tool                                 │
├────────────────────────────────────────────────────────────────────────┤
│                                                                        │
│  ┌─────────────┐                                                      │
│  │ Developer   │                                                      │
│  │ Workstation │                                                      │
│  │             │                                                      │
│  │ ┌─────────┐ │                                                      │
│  │ │ CLI     │ │──────────▶ Vault/KMS                                │
│  │ │ Binary  │ │                                                      │
│  │ └─────────┘ │                                                      │
│  │ ┌─────────┐ │                                                      │
│  │ │ Local   │ │                                                      │
│  │ │ Cache   │ │                                                      │
│  │ │ (sled)  │ │                                                      │
│  │ └─────────┘ │                                                      │
│  └─────────────┘                                                      │
│                                                                        │
│  Target: Individual developers, CI/CD pipelines                       │
│  Latency: <10ms (local), <100ms (vault)                               │
└────────────────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────────────────┐
│                  Microservice API Server                               │
├────────────────────────────────────────────────────────────────────────┤
│                                                                        │
│  ┌──────────────────────────────────────────────────┐                 │
│  │            Kubernetes Cluster                    │                 │
│  │                                                  │                 │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐         │                 │
│  │  │ API     │  │ API     │  │ API     │         │                 │
│  │  │ Server  │  │ Server  │  │ Server  │ (3+)    │                 │
│  │  │ Pod     │  │ Pod     │  │ Pod     │         │                 │
│  │  └────┬────┘  └────┬────┘  └────┬────┘         │                 │
│  │       └────────────┼────────────┘               │                 │
│  │                    │                            │                 │
│  │  ┌─────────────────▼─────────────────┐         │                 │
│  │  │       Load Balancer (Ingress)     │         │                 │
│  │  └───────────────────────────────────┘         │                 │
│  │                    │                            │                 │
│  │  ┌─────────────────▼─────────────────┐         │                 │
│  │  │         Redis Cluster             │         │                 │
│  │  │       (Distributed Cache)         │         │                 │
│  │  └───────────────────────────────────┘         │                 │
│  │                    │                            │                 │
│  │  ┌─────────────────▼─────────────────┐         │                 │
│  │  │      PostgreSQL Primary           │         │                 │
│  │  │    (Metadata, Audit Logs)         │         │                 │
│  │  └───────────────────────────────────┘         │                 │
│  └──────────────────────────────────────────────────┘                 │
│                    │                                                  │
│                    ▼                                                  │
│           Vault/KMS (Secrets)                                         │
│                                                                        │
│  Target: Enterprise-wide, 10,000+ clients                             │
│  Throughput: 50,000+ req/s (cached)                                   │
│  Latency: p99 <5ms (cached), <50ms (vault)                            │
└────────────────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────────────────┐
│                    Sidecar Integration Mode                            │
├────────────────────────────────────────────────────────────────────────┤
│                                                                        │
│  ┌────────────────────────────────────────────────┐                   │
│  │            Application Pod                     │                   │
│  │                                                │                   │
│  │  ┌─────────────────┐   ┌─────────────────┐   │                   │
│  │  │ Application     │   │ Config-Manager  │   │                   │
│  │  │ Container       │   │ Sidecar         │   │                   │
│  │  │                 │   │                 │   │                   │
│  │  │ ┌─────────────┐ │   │ ┌─────────────┐ │   │                   │
│  │  │ │ App Process │ │   │ │ Sync Agent  │ │   │                   │
│  │  │ └──────┬──────┘ │   │ └──────┬──────┘ │   │                   │
│  │  │        │        │   │        │        │   │                   │
│  │  └────────┼────────┘   └────────┼────────┘   │                   │
│  │           │                     │            │                   │
│  │           │   Unix Socket /     │            │                   │
│  │           │   localhost HTTP    │            │                   │
│  │           └─────────────────────┘            │                   │
│  │                                  │            │                   │
│  │  ┌──────────────────────────────▼──────────┐ │                   │
│  │  │     Shared emptyDir Volume              │ │                   │
│  │  │     (Config Cache)                      │ │                   │
│  │  └─────────────────────────────────────────┘ │                   │
│  └────────────────────────────────────────────────┘                   │
│                         │                                             │
│                         ▼                                             │
│            Central API Server (for sync)                              │
│                         │                                             │
│                         ▼                                             │
│                    Vault/KMS                                          │
│                                                                        │
│  Target: Ultra-low latency apps (p99 <5ms)                            │
│  Resource: 64-256Mi memory, 50-200m CPU                               │
│  Latency: p99 <1ms (cached)                                           │
└────────────────────────────────────────────────────────────────────────┘
```

## Component Layer Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        PRESENTATION LAYER                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌──────────────────┐  ┌──────────────────┐  ┌───────────────────┐   │
│  │   REST API       │  │   gRPC API       │  │   CLI Interface   │   │
│  │   (axum)         │  │   (tonic)        │  │   (clap)          │   │
│  │                  │  │                  │  │                   │   │
│  │ • Auth/AuthZ     │  │ • ConfigService  │  │ • config get/set  │   │
│  │ • Rate limiting  │  │ • WatchService   │  │ • secret rotate   │   │
│  │ • Validation     │  │ • ValidateService│  │ • policy validate │   │
│  │ • /health        │  │ • AuditService   │  │ • audit query     │   │
│  │ • /metrics       │  │                  │  │ • TUI (ratatui)   │   │
│  └──────────────────┘  └──────────────────┘  └───────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
                                   │
┌─────────────────────────────────────────────────────────────────────────┐
│                       APPLICATION LAYER                                 │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────┐      │
│  │              Configuration Engine                           │      │
│  │  ┌────────────┐ ┌─────────────┐ ┌──────────┐ ┌──────────┐ │      │
│  │  │ConfigResolver│TemplateEngine│ │Validator│ │VersionMgr│ │      │
│  │  └────────────┘ └─────────────┘ └──────────┘ └──────────┘ │      │
│  └─────────────────────────────────────────────────────────────┘      │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────┐      │
│  │              Secrets Manager                                │      │
│  │  ┌─────────────┐ ┌──────────┐ ┌────────────────┐          │      │
│  │  │EncryptionSvc│ │ KeyMgr   │ │ SecretStore    │          │      │
│  │  │AES-GCM/ChaCha│ │KEK/Rotate│ │In-memory secure│          │      │
│  │  └─────────────┘ └──────────┘ └────────────────┘          │      │
│  └─────────────────────────────────────────────────────────────┘      │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────┐      │
│  │         Policy Engine Integration                           │      │
│  │  ┌────────────┐ ┌──────────────┐ ┌──────────────┐         │      │
│  │  │RBACEnforcer│ │PolicyValidator│ │PolicyClient  │         │      │
│  │  └────────────┘ └──────────────┘ └──────────────┘         │      │
│  └─────────────────────────────────────────────────────────────┘      │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────┐      │
│  │              Audit Logger                                   │      │
│  │  ┌──────────┐ ┌───────────┐ ┌──────────┐ ┌─────────┐      │      │
│  │  │Event Cap.│ │Persistence│ │Integrity │ │QueryAPI │      │      │
│  │  │          │ │PostgreSQL │ │Merkle Tree│          │      │      │
│  │  └──────────┘ └───────────┘ └──────────┘ └─────────┘      │      │
│  └─────────────────────────────────────────────────────────────┘      │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────┐      │
│  │              Cache Manager                                  │      │
│  │  ┌──────┐  ┌──────┐  ┌──────┐                              │      │
│  │  │ L1   │  │ L2   │  │ L3   │                              │      │
│  │  │Memory│  │Redis │  │Disk  │                              │      │
│  │  │ LRU  │  │Distrib│ │sled  │                              │      │
│  │  └──────┘  └──────┘  └──────┘                              │      │
│  └─────────────────────────────────────────────────────────────┘      │
└─────────────────────────────────────────────────────────────────────────┘
                                   │
┌─────────────────────────────────────────────────────────────────────────┐
│                       INTEGRATION LAYER                                 │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐      │
│  │  Vault     │  │ AWS KMS    │  │ GCP KMS    │  │ Azure KV   │      │
│  │  Adapter   │  │ Adapter    │  │ Adapter    │  │ Adapter    │      │
│  │ (vaultrs)  │  │(rusoto_kms)│  │(google-kms)│  │(azure_kv)  │      │
│  └────────────┘  └────────────┘  └────────────┘  └────────────┘      │
│                                                                         │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐      │
│  │  Policy    │  │Governance  │  │Observatory │  │ Edge Agent │      │
│  │  Engine    │  │ Dashboard  │  │ (Metrics)  │  │Integration │      │
│  │  (gRPC)    │  │(REST/WS)   │  │(Prometheus)│  │(gRPC)      │      │
│  └────────────┘  └────────────┘  └────────────┘  └────────────┘      │
│                                                                         │
│  ┌────────────┐                                                        │
│  │  Auto      │                                                        │
│  │  Optimizer │                                                        │
│  │  (REST)    │                                                        │
│  └────────────┘                                                        │
└─────────────────────────────────────────────────────────────────────────┘
                                   │
┌─────────────────────────────────────────────────────────────────────────┐
│                          DATA LAYER                                     │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐          │
│  │   Vault/KMS    │  │  PostgreSQL    │  │   Redis/Sled   │          │
│  │   (Primary)    │  │  (Metadata)    │  │    (Cache)     │          │
│  │                │  │                │  │                │          │
│  │ • Secrets      │  │ • Configs meta │  │ • L2: Redis    │          │
│  │ • Encrypted    │  │ • Versions     │  │ • L3: Sled     │          │
│  │   configs      │  │ • Audit logs   │  │ • Pub/Sub      │          │
│  │ • KEKs         │  │ • RBAC data    │  │ • Invalidation │          │
│  └────────────────┘  └────────────────┘  └────────────────┘          │
└─────────────────────────────────────────────────────────────────────────┘
```

## Data Flow: Config Read Path

```
┌─────────┐
│ Client  │
└────┬────┘
     │ GET /configs/prod/ml-service/db-url
     ▼
┌─────────────────────┐
│ REST API (axum)     │
│ 1. Authenticate     │
│ 2. Authorize        │────────▶ Policy Engine (RBAC check)
└─────────┬───────────┘
          │
          ▼
┌─────────────────────┐
│ Cache Manager       │
│ 3. Check L1 (mem)   │────────▶ HIT? Return ──────┐
└─────────┬───────────┘                            │
          │ MISS                                   │
          ▼                                        │
┌─────────────────────┐                            │
│ 4. Check L2 (Redis) │────────▶ HIT? Store L1 ───┤
└─────────┬───────────┘              Return        │
          │ MISS                                   │
          ▼                                        │
┌─────────────────────┐                            │
│ 5. Vault Adapter    │                            │
│    Fetch from Vault │                            │
└─────────┬───────────┘                            │
          │                                        │
          ▼                                        │
┌─────────────────────┐                            │
│ 6. Decrypt (if enc) │                            │
│ 7. Validate schema  │                            │
│ 8. Store L1 + L2    │                            │
└─────────┬───────────┘                            │
          │                                        │
          └────────────────────────────────────────┤
                                                   │
          ┌────────────────────────────────────────┘
          │
          ▼
┌─────────────────────┐
│ 9. Log audit event  │──────▶ Audit Logger (async)
└─────────┬───────────┘
          │
          ▼
     ┌─────────┐
     │ Response│
     └─────────┘
```

## Data Flow: Config Write Path

```
┌─────────┐
│ Client  │
└────┬────┘
     │ POST /configs/prod/ml-service/db-url
     │ Body: { "value": "postgres://..." }
     ▼
┌─────────────────────┐
│ REST API (axum)     │
│ 1. Authenticate     │
│ 2. Authorize        │────────▶ Policy Engine (Write permission check)
└─────────┬───────────┘
          │
          ▼
┌─────────────────────┐
│ Config Engine       │
│ 3. Validate schema  │────────▶ Policy Engine (Schema validation)
└─────────┬───────────┘
          │
          ▼
┌─────────────────────┐
│ Secrets Manager     │
│ 4. Encrypt (if sec) │
│    • Generate DEK   │
│    • Encrypt DEK    │────────▶ Cloud KMS
│    • Encrypt value  │
└─────────┬───────────┘
          │
          ▼
┌─────────────────────┐
│ 5. Vault Adapter    │
│    Write to Vault   │────────▶ Vault/KMS
└─────────┬───────────┘
          │
          ▼
┌─────────────────────┐
│ 6. Metadata Store   │
│    Save version     │────────▶ PostgreSQL
│    Save metadata    │
└─────────┬───────────┘
          │
          ▼
┌─────────────────────┐
│ 7. Cache Invalidate │
│    Broadcast msg    │────────▶ Redis Pub/Sub ──▶ All instances
└─────────┬───────────┘
          │
          ▼
┌─────────────────────┐
│ 8. Audit Logger     │────────▶ PostgreSQL (async)
│    Record write     │
└─────────┬───────────┘
          │
          ▼
┌─────────────────────┐
│ 9. Notify Dashboard │────────▶ Governance Dashboard (WebSocket)
└─────────┬───────────┘
          │
          ▼
     ┌─────────┐
     │ Response│
     │ 201 OK  │
     └─────────┘
```

## Namespace Hierarchy

```
/ (root)
│
├── production/
│   │
│   ├── ml-service/
│   │   ├── inference/
│   │   │   ├── model.config
│   │   │   ├── scaling.yaml
│   │   │   └── secrets/
│   │   │       ├── api-key
│   │   │       └── db-credentials
│   │   │
│   │   ├── training/
│   │   │   ├── hyperparameters.json
│   │   │   └── dataset.config
│   │   │
│   │   └── monitoring/
│   │       ├── prometheus.yaml
│   │       └── alerting.rules
│   │
│   ├── api-gateway/
│   │   ├── routes.yaml
│   │   ├── rate-limits.json
│   │   └── secrets/
│   │       └── jwt-signing-key
│   │
│   └── data-pipeline/
│       ├── kafka.config
│       └── spark.yaml
│
├── staging/
│   ├── ml-service/
│   └── api-gateway/
│
└── development/
    ├── ml-service/
    └── api-gateway/

Environment Inheritance:
  base ──▶ development ──▶ staging ──▶ production
```

## Security Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                       SECURITY LAYERS                               │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│  AUTHENTICATION LAYER                                               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐          │
│  │   JWT    │  │  mTLS    │  │ API Key  │  │ OAuth2   │          │
│  │  Tokens  │  │  Certs   │  │          │  │  /OIDC   │          │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘          │
└─────────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────────┐
│  AUTHORIZATION LAYER (RBAC + ABAC)                                  │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌───────────────────────────────────────────────────────────┐    │
│  │  Policy Engine Integration                                │    │
│  │  • Role-based permissions                                 │    │
│  │  • Namespace-scoped access                                │    │
│  │  • Attribute-based conditions                             │    │
│  │  • Permission caching (5 min TTL)                         │    │
│  └───────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────────┐
│  ENCRYPTION LAYER                                                   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────┐          ┌─────────────────┐                 │
│  │  In Transit     │          │  At Rest        │                 │
│  │  • TLS 1.3      │          │  • AES-256-GCM  │                 │
│  │  • mTLS         │          │  • ChaCha20     │                 │
│  │  • PFS          │          │  • Envelope Enc │                 │
│  └─────────────────┘          └─────────────────┘                 │
│                                                                     │
│  ┌───────────────────────────────────────────────────────────┐    │
│  │  Key Management                                           │    │
│  │  • KEK: Cloud KMS (AWS/GCP/Azure)                        │    │
│  │  • DEK: Per-config data encryption key                   │    │
│  │  • Auto-rotation: Every 90 days                          │    │
│  │  • Key versioning: Old keys for decryption              │    │
│  └───────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────────┐
│  AUDIT LAYER                                                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌───────────────────────────────────────────────────────────┐    │
│  │  Immutable Audit Logs                                     │    │
│  │  • All operations logged                                  │    │
│  │  • Cryptographic integrity (Merkle trees)                 │    │
│  │  • Tamper-evident storage                                 │    │
│  │  • 7-year retention                                       │    │
│  │  • Compliance: SOC2, PCI, HIPAA, GDPR, ISO 27001        │    │
│  └───────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────┘
```

## Multi-Region Deployment

```
┌────────────────────────────────────────────────────────────────────┐
│                     Global Multi-Region Architecture               │
└────────────────────────────────────────────────────────────────────┘

     ┌──────────────────────────────────────────────────────────┐
     │                    Global Load Balancer                  │
     │                   (DNS-based routing)                    │
     └───────┬────────────────────┬────────────────────┬────────┘
             │                    │                    │
    ┌────────▼────────┐  ┌────────▼────────┐  ┌────────▼────────┐
    │   us-west-2     │  │   eu-west-1     │  │   ap-south-1    │
    │   (Oregon)      │  │   (Ireland)     │  │   (Mumbai)      │
    └─────────────────┘  └─────────────────┘  └─────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│  Region: us-west-2 (Oregon)                                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────────────────────────────────┐                   │
│  │  Config-Manager API Cluster                │                   │
│  │  ┌──────┐  ┌──────┐  ┌──────┐             │                   │
│  │  │ Pod  │  │ Pod  │  │ Pod  │  (3+)       │                   │
│  │  └──────┘  └──────┘  └──────┘             │                   │
│  └─────────────────────────────────────────────┘                   │
│                         │                                           │
│  ┌──────────────────────┼──────────────────────┐                   │
│  │  Regional Services   │                      │                   │
│  │  ┌─────────┐  ┌──────▼──────┐  ┌────────┐ │                   │
│  │  │ Vault   │  │ Redis       │  │ Postgres│ │                   │
│  │  │(Primary)│  │ (Primary)   │  │(Primary)│ │                   │
│  │  └─────────┘  └─────────────┘  └────────┘ │                   │
│  └────────────────────────────────────────────┘                   │
│           │                 │            │                          │
│           └─────Replication─┴────────────┘                          │
│                             │                                       │
└─────────────────────────────┼───────────────────────────────────────┘
                              │
┌─────────────────────────────┼───────────────────────────────────────┐
│  Region: eu-west-1 (Ireland)│                                       │
├─────────────────────────────┼───────────────────────────────────────┤
│                             │                                       │
│  ┌─────────────────────────────────────────────┐                   │
│  │  Config-Manager API Cluster                │                   │
│  │  ┌──────┐  ┌──────┐  ┌──────┐             │                   │
│  │  │ Pod  │  │ Pod  │  │ Pod  │  (3+)       │                   │
│  │  └──────┘  └──────┘  └──────┘             │                   │
│  └─────────────────────────────────────────────┘                   │
│                         │                                           │
│  ┌──────────────────────┼──────────────────────┐                   │
│  │  Regional Services   │                      │                   │
│  │  ┌─────────┐  ┌──────▼──────┐  ┌────────┐ │                   │
│  │  │ Vault   │  │ Redis       │  │ Postgres│ │                   │
│  │  │(Replica)│  │ (Replica)   │  │(Replica)│ │                   │
│  │  └─────────┘  └─────────────┘  └────────┘ │                   │
│  └────────────────────────────────────────────┘                   │
│           │                 │            │                          │
│           └─────Replication─┴────────────┘                          │
└─────────────────────────────────────────────────────────────────────┘

Replication Strategy:
  • Vault: Vault Enterprise replication (async)
  • Redis: Redis replication (async)
  • PostgreSQL: Streaming replication (async)
  • Config writes: Primary region only (us-west-2)
  • Config reads: Local region (low latency)
  • Consistency: Eventual (replication lag <1s)
```

## Caching Strategy

```
┌────────────────────────────────────────────────────────────────────┐
│                     Multi-Tier Cache Architecture                  │
└────────────────────────────────────────────────────────────────────┘

Client Request
     │
     ▼
┌─────────────────────────────────────────┐
│  L1 Cache: In-Memory (per-instance)     │  ◀─── Fastest (μs)
│  • LRU eviction                         │
│  • Size: 100MB                          │
│  • TTL: 5 minutes                       │
│  • Hit ratio target: 70%                │
└─────────┬───────────────────────────────┘
          │ MISS
          ▼
┌─────────────────────────────────────────┐
│  L2 Cache: Redis (cluster-wide)         │  ◀─── Fast (ms)
│  • Distributed cache                    │
│  • Size: 10GB                           │
│  • TTL: 10 minutes                      │
│  • Hit ratio target: 25%                │
│  • Pub/Sub for invalidation             │
└─────────┬───────────────────────────────┘
          │ MISS
          ▼
┌─────────────────────────────────────────┐
│  L3 Cache: Local Disk (sled)            │  ◀─── Medium (ms)
│  • Embedded database                    │
│  • Size: 1GB                            │
│  • Persistent across restarts           │
│  • Hit ratio target: 4%                 │
└─────────┬───────────────────────────────┘
          │ MISS
          ▼
┌─────────────────────────────────────────┐
│  Vault/KMS (source of truth)            │  ◀─── Slowest (10-50ms)
│  • Authoritative storage                │
│  • Encrypted at rest                    │
│  • Hit ratio: 1% (cache misses only)    │
└─────────────────────────────────────────┘

Cache Invalidation:
  1. Config update detected
  2. Broadcast invalidation via Redis Pub/Sub
  3. All instances clear L1, L2, L3 for that key
  4. Next request fetches fresh data from Vault
  5. Repopulate all cache tiers

Overall Cache Hit Ratio: 99% (target >95%)
```

---

## Deployment Decision Matrix

| Requirement | CLI | API Server | Sidecar | Hybrid |
|-------------|-----|------------|---------|--------|
| **Low latency (<5ms)** | ✗ | ✓ (cached) | ✓✓✓ | ✓✓✓ |
| **High throughput (>50K req/s)** | ✗ | ✓✓✓ | ✗ | ✓✓✓ |
| **Offline support** | ✓✓✓ | ✗ | ✓✓ | ✓ |
| **Multi-tenant** | ✗ | ✓✓✓ | ✓ | ✓✓✓ |
| **Simple deployment** | ✓✓✓ | ✓ | ✓ | ✗ |
| **Cost efficiency** | ✓✓✓ | ✓ | ✓✓ | ✓✓ |
| **Centralized audit** | ✗ | ✓✓✓ | ✓✓ | ✓✓✓ |
| **Dev/local use** | ✓✓✓ | ✗ | ✗ | ✓ |
| **Production scale** | ✗ | ✓✓✓ | ✓✓ | ✓✓✓ |
| **GitOps integration** | ✓ | ✓✓✓ | ✓ | ✓✓✓ |

Legend: ✗ Not suitable | ✓ Suitable | ✓✓ Good | ✓✓✓ Excellent

---

**Document Version:** 1.0.0
**Last Updated:** 2025-11-21
**Purpose:** Visual reference for LLM-Config-Manager architecture
