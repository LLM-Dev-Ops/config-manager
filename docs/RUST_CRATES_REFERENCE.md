# Rust Crates Quick Reference - LLM-Config-Manager

**Version:** 1.0.0
**Date:** 2025-11-21
**Purpose:** Quick reference for all recommended production-ready Rust crates

---

## Cargo.toml Dependencies

```toml
[dependencies]
# === Cryptography and Security ===
# Core cryptography
ring = "0.17"                          # Core crypto operations, AEAD, key derivation, RNG
aes-gcm = "0.10"                       # AES-256-GCM encryption (RustCrypto)
chacha20poly1305 = "0.10"              # ChaCha20-Poly1305 AEAD (ARM/embedded)
rustls = "0.23"                        # TLS 1.3 implementation
rustls-native-certs = "0.7"            # Native certificate loading

# Password hashing and key derivation
argon2 = "0.5"                         # Argon2id password hashing (OWASP recommended)
pbkdf2 = "0.12"                        # PBKDF2 key derivation (FIPS-140 compliant)
hkdf = "0.12"                          # HKDF key derivation

# Digital signatures and certificates
ed25519-dalek = "2.1"                  # Ed25519 signatures (audit log integrity)
rsa = "0.9"                            # RSA signatures/encryption (legacy compatibility)
x509-parser = "0.16"                   # X.509 certificate parsing
webpki = "0.22"                        # Web PKI certificate validation

# Hashing
sha2 = "0.10"                          # SHA-256, SHA-512 hashing

# === Secrets Backend Integration ===
# HashiCorp Vault
vaultrs = "0.7"                        # Async Vault client (KV v1/v2, Transit, AppRole, etc.)

# AWS
aws-config = "1.0"                     # AWS SDK configuration
aws-sdk-kms = "1.0"                    # AWS KMS client

# Azure
azure_identity = "0.20"                # Azure authentication (Managed Identity)
azure_security_keyvault_keys = "0.20" # Azure Key Vault keys
azure_security_keyvault_secrets = "0.20" # Azure Key Vault secrets

# GCP (community-maintained)
google-cloud-kms = "0.7"               # GCP Cloud KMS client
gcloud-sdk = "0.25"                    # Alternative GCP SDK (gRPC-based)

# === Serialization and Configuration ===
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"                     # JSON serialization
toml = "0.8"                           # TOML serialization
serde-yaml-ng = "0.10"                 # YAML serialization (maintained fork)
config = "0.14"                        # Layered configuration management
figment = "0.10"                       # Advanced config aggregation

# === Validation ===
jsonschema = "0.18"                    # JSON Schema validation (drafts 4/6/7/2019-09/2020-12)
validator = { version = "0.18", features = ["derive"] } # Derive-based validation
serde_valid = "0.22"                   # Serde-integrated validation

# === Access Control and RBAC ===
casbin = "2.3"                         # RBAC/ABAC authorization library

# === HTTP/gRPC Servers ===
# HTTP frameworks
axum = "0.7"                           # Primary HTTP framework (modern, type-safe)
actix-web = "4.5"                      # Alternative (extreme throughput scenarios)
tower = "0.4"                          # Service abstraction (rate limiting, timeouts)
tower-http = "0.5"                     # HTTP middleware (CORS, compression, auth)

# gRPC
tonic = "0.11"                         # gRPC framework
prost = "0.12"                         # Protocol Buffers implementation

# === Database and Storage ===
# SQL
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "postgres", "migrate"] }

# Key-Value
sled = "0.34"                          # Embedded database (local caching)
redis = { version = "0.24", features = ["tokio-comp", "connection-manager"] }

# === Audit Logging and Observability ===
# Structured logging (Modern standard for 2025)
tracing = "0.1"                        # Structured logging and distributed tracing
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
tracing-appender = "0.2"               # Non-blocking file appender
tracing-opentelemetry = "0.22"         # OpenTelemetry integration

# Metrics
metrics = "0.22"                       # Application metrics collection
metrics-exporter-prometheus = "0.13"   # Prometheus metrics export

# === CLI and TUI ===
clap = { version = "4.5", features = ["derive"] } # Command-line parsing
ratatui = "0.26"                       # Terminal UI framework
crossterm = "0.27"                     # Cross-platform terminal manipulation
indicatif = "0.17"                     # Progress bars and spinners

# === Async Runtime ===
tokio = { version = "1.35", features = ["full"] } # Async runtime
async-trait = "0.1"                    # Async traits

# === Error Handling ===
anyhow = "1.0"                         # Flexible error handling
thiserror = "1.0"                      # Error derive macros

# === Utilities ===
uuid = { version = "1.6", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
base64 = "0.21"
rand = "0.8"
zeroize = "1.7"                        # Secure memory zeroing

# === Testing and Security ===
[dev-dependencies]
mockall = "0.12"                       # Mocking framework
wiremock = "0.6"                       # HTTP mocking
proptest = "1.4"                       # Property-based testing
```

---

## Cargo.toml Build Dependencies

```toml
[build-dependencies]
tonic-build = "0.11"                   # gRPC code generation
```

---

## Security and Development Tools

### CI/CD Security Pipeline

```toml
# Install with: cargo install <tool>

cargo-audit = "0.20"                   # Dependency vulnerability scanning
cargo-deny = "0.14"                    # Dependency policy enforcement
cargo-clippy = "latest"                # Linting (built-in)
cargo-outdated = "0.14"                # Check for outdated dependencies
cargo-update = "13.3"                  # Update installed tools
```

### External Security Tools

```bash
# Static analysis
semgrep                                # Static analysis for vulnerabilities

# Container security
trivy                                  # Container image scanning
grype                                  # Vulnerability scanner

# Secret scanning
trufflehog                             # Secret scanning in git history
gitleaks                               # Git secret scanner
```

---

## Cargo.toml Configuration

### .cargo/audit.toml

```toml
[advisories]
db-path = "~/.cargo/advisory-db"
db-urls = ["https://github.com/rustsec/advisory-db"]
vulnerability = "deny"
unmaintained = "warn"
unsound = "warn"
yanked = "deny"

[licenses]
unlicensed = "deny"
copyleft = "deny"
allow = [
    "MIT",
    "Apache-2.0",
    "BSD-3-Clause",
    "ISC",
    "Unicode-DFS-2016",
]

[bans]
multiple-versions = "warn"
wildcards = "deny"
```

### .cargo/deny.toml

```toml
[advisories]
vulnerability = "deny"
unmaintained = "warn"
unsound = "warn"
yanked = "deny"

[licenses]
unlicensed = "deny"
allow = [
    "MIT",
    "Apache-2.0",
    "BSD-3-Clause",
    "ISC",
    "Unicode-DFS-2016",
]
copyleft = "deny"

[bans]
multiple-versions = "warn"
wildcards = "deny"
deny = [
    # Add specific crates to ban if needed
]

[sources]
unknown-registry = "deny"
unknown-git = "deny"
```

---

## Crate Categories and Use Cases

### Cryptography - When to Use What

| Use Case | Crate | Rationale |
|----------|-------|-----------|
| **AEAD Encryption** | `aes-gcm` | Hardware acceleration (AES-NI), NIST recommended |
| **AEAD (ARM/Embedded)** | `chacha20poly1305` | Better software performance without AES-NI |
| **TLS/HTTPS** | `rustls` | Memory-safe, no C dependencies, excellent performance |
| **Password Hashing** | `argon2` | OWASP recommended, GPU-resistant |
| **Key Derivation (FIPS)** | `pbkdf2` | FIPS-140 compliant for regulatory requirements |
| **Key Derivation (Modern)** | `hkdf` | Extract-and-expand, multiple keys from one master |
| **Digital Signatures** | `ed25519-dalek` | Fast, secure, audit log integrity |
| **Hashing** | `sha2` | SHA-256, SHA-512 for checksums and Merkle trees |
| **Random Generation** | `ring::rand` | Cryptographically secure RNG |

### HTTP/gRPC - When to Use What

| Scenario | Framework | Rationale |
|----------|-----------|-----------|
| **Modern REST API** | `axum` | Type-safe, Tower ecosystem, lower resource usage |
| **Extreme Throughput** | `actix-web` | Best benchmark performance (>100K req/s) |
| **Service-to-Service** | `tonic` | gRPC with async/await, streaming, code generation |
| **Middleware** | `tower` + `tower-http` | Rate limiting, timeouts, CORS, compression |

### Logging - Why Tracing Over Log

| Feature | `log` | `tracing` |
|---------|-------|-----------|
| **Async Support** | Basic | Excellent (async-first) |
| **Structured Fields** | Limited | Native support |
| **Context Propagation** | Manual | Automatic (spans) |
| **Performance** | Good | Better (async-optimized) |
| **Modern Services** | Legacy standard | 2025 standard |
| **Use For** | Libraries (compatibility) | Applications (functionality) |

### Database - When to Use What

| Use Case | Crate/Database | Rationale |
|----------|----------------|-----------|
| **Metadata/Audit** | `sqlx` + PostgreSQL | ACID, compile-time query verification |
| **Distributed Cache** | `redis` | Fast, pub/sub, cluster support |
| **Local Cache** | `sled` | Pure Rust, embedded, ACID, no dependencies |

---

## Version Compatibility Matrix

### Tested Rust Versions
- **MSRV (Minimum):** 1.70.0
- **Recommended:** 1.75.0+
- **CI Testing:** 1.75.0, 1.76.0, stable, nightly

### Platform Support
- **Linux:** x86_64, aarch64 (primary)
- **macOS:** x86_64, aarch64 (development)
- **Windows:** x86_64 (limited support)

---

## Security Update Policy

### Critical Security Updates
- **Timeline:** Within 48 hours of advisory
- **Process:**
  1. Automated cargo-audit detection
  2. Security team notification
  3. Dependency update and testing
  4. Emergency release if in production
  5. Post-incident analysis

### Regular Updates
- **Minor Updates:** Monthly review
- **Major Updates:** Quarterly review
- **Breaking Changes:** Planned migration with deprecation period

---

## Common Patterns and Examples

### Envelope Encryption with AWS KMS

```rust
use aws_sdk_kms::Client as KmsClient;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use aes_gcm::aead::{Aead, OsRng};

// 1. Generate DEK
let dek_response = kms_client
    .generate_data_key()
    .key_id("arn:aws:kms:us-east-1:123456789:key/...")
    .key_spec(DataKeySpec::Aes256)
    .send()
    .await?;

// 2. Encrypt data with DEK
let cipher = Aes256Gcm::new(GenericArray::from_slice(plaintext_dek));
let nonce = Nonce::from_slice(&nonce_bytes);
let ciphertext = cipher.encrypt(nonce, plaintext)?;

// 3. Store encrypted_dek + ciphertext
```

### Structured Audit Logging with Tracing

```rust
use tracing::{info, instrument};

#[instrument(
    name = "config_read",
    fields(
        user_id = %user_id,
        tenant_id = %tenant_id,
        namespace = %namespace,
        result = tracing::field::Empty,
    )
)]
async fn read_config(
    user_id: &str,
    tenant_id: &str,
    namespace: &str,
) -> Result<ConfigValue> {
    info!("Configuration read requested");

    let value = fetch_config(namespace).await?;

    tracing::Span::current().record("result", "success");
    Ok(value)
}
```

### RBAC with Casbin

```rust
use casbin::prelude::*;

// Initialize enforcer
let mut enforcer = Enforcer::new("model.conf", "policy.csv").await?;

// Check authorization
if enforcer.enforce(("alice", "tenant1", "configs/prod/db", "write"))? {
    // Allow access
} else {
    // Deny access
}
```

### JSON Schema Validation

```rust
use jsonschema::{Draft, JSONSchema};

let schema = json!({
    "type": "object",
    "properties": {
        "api_key": { "type": "string", "pattern": "^sk-[a-zA-Z0-9]{48}$" }
    },
    "required": ["api_key"]
});

let compiled = JSONSchema::options()
    .with_draft(Draft::Draft7)
    .compile(&schema)?;

if let Err(errors) = compiled.validate(&config) {
    // Handle validation errors
}
```

---

## Performance Considerations

### Crate Benchmarks (Relative Performance)

| Operation | Crate | Performance Tier |
|-----------|-------|------------------|
| **AES-GCM Encryption** | `ring` | Fastest (hardware accelerated) |
| **ChaCha20 Encryption** | `chacha20poly1305` | Fast (software optimized) |
| **Password Hashing** | `argon2` | Slow by design (security) |
| **HTTP Throughput** | `actix-web` | Fastest |
| **HTTP Ergonomics** | `axum` | Fast + excellent DX |
| **Async Runtime** | `tokio` | Industry standard |

---

## Maintenance and Support Status

### Actively Maintained (2025)
- ring, rustls, argon2, ed25519-dalek
- tracing, metrics
- axum, tonic
- sqlx, redis
- casbin
- aws-sdk-*, azure_security_keyvault_*

### Community Maintained
- google-cloud-kms, gcloud-sdk
- sled (stable, but slow updates)

### Deprecated (Do Not Use)
- sodiumoxide (use ring instead)
- serde_yaml (use serde-yaml-ng instead)

---

## Additional Resources

### Documentation
- **Rust Cryptography:** <https://rustcrypto.org/>
- **RustSec Advisory DB:** <https://rustsec.org/>
- **Tokio Guide:** <https://tokio.rs/tokio/tutorial>
- **Tracing Guide:** <https://docs.rs/tracing>

### Security
- **OWASP:** <https://owasp.org/www-project-cheat-sheets/>
- **NIST:** <https://csrc.nist.gov/>
- **CWE Top 25:** <https://cwe.mitre.org/top25/>

---

**Document Version:** 1.0.0
**Last Updated:** 2025-11-21
**Author:** Security Architect Agent
**Status:** Complete - Ready for Implementation
