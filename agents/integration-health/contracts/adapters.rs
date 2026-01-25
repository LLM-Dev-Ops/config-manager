//! External adapter types and configuration
//!
//! Defines the adapters that can be health-checked.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Adapter configuration for health check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterConfig {
    /// Unique adapter identifier
    pub id: String,

    /// Adapter type
    pub adapter_type: AdapterType,

    /// Connection endpoint
    pub endpoint: String,

    /// Authentication configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,

    /// Custom health check path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_path: Option<String>,

    /// Additional properties
    #[serde(default)]
    pub properties: HashMap<String, String>,
}

/// Supported adapter types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterType {
    /// AWS Systems Manager Parameter Store
    AwsSsm,
    /// AWS Secrets Manager
    AwsSecretsManager,
    /// Google Cloud Secret Manager
    GcpSecretManager,
    /// Azure Key Vault
    AzureKeyVault,
    /// HashiCorp Vault
    HashicorpVault,
    /// Redis
    Redis,
    /// PostgreSQL
    Postgres,
    /// MySQL
    Mysql,
    /// HTTP endpoint
    Http,
    /// gRPC service
    Grpc,
    /// Kafka
    Kafka,
    /// RabbitMQ
    Rabbitmq,
    /// S3-compatible storage
    S3,
    /// Generic TCP
    Tcp,
    /// Custom adapter
    Custom,
}

impl AdapterType {
    /// Get default health check path
    pub fn default_health_path(&self) -> Option<&'static str> {
        match self {
            AdapterType::Http => Some("/health"),
            AdapterType::Grpc => Some("grpc.health.v1.Health/Check"),
            AdapterType::HashicorpVault => Some("/v1/sys/health"),
            _ => None,
        }
    }

    /// Get default port
    pub fn default_port(&self) -> Option<u16> {
        match self {
            AdapterType::Redis => Some(6379),
            AdapterType::Postgres => Some(5432),
            AdapterType::Mysql => Some(3306),
            AdapterType::Http => Some(80),
            AdapterType::Grpc => Some(50051),
            AdapterType::Kafka => Some(9092),
            AdapterType::Rabbitmq => Some(5672),
            AdapterType::HashicorpVault => Some(8200),
            _ => None,
        }
    }
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthConfig {
    /// No authentication
    None,

    /// API key/token
    ApiKey {
        header: Option<String>,
        key_ref: String,
    },

    /// Basic auth
    Basic {
        username_ref: String,
        password_ref: String,
    },

    /// Bearer token
    Bearer { token_ref: String },

    /// mTLS
    Mtls {
        cert_ref: String,
        key_ref: String,
        ca_ref: Option<String>,
    },

    /// AWS credentials
    AwsCredentials {
        access_key_ref: Option<String>,
        secret_key_ref: Option<String>,
        region: String,
    },

    /// GCP service account
    GcpServiceAccount { credentials_ref: String },

    /// Azure credentials
    AzureCredentials {
        client_id_ref: String,
        client_secret_ref: String,
        tenant_id: String,
    },
}

/// Adapter health check configuration preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterPreset {
    /// Preset name
    pub name: String,

    /// Base adapter type
    pub adapter_type: AdapterType,

    /// Default endpoint pattern
    pub endpoint_pattern: String,

    /// Required properties
    pub required_properties: Vec<String>,

    /// Default auth type
    pub default_auth: Option<AuthConfig>,
}

impl AdapterPreset {
    /// AWS SSM preset
    pub fn aws_ssm(region: &str) -> AdapterConfig {
        AdapterConfig {
            id: format!("aws-ssm-{}", region),
            adapter_type: AdapterType::AwsSsm,
            endpoint: format!("ssm.{}.amazonaws.com", region),
            auth: Some(AuthConfig::AwsCredentials {
                access_key_ref: None,
                secret_key_ref: None,
                region: region.to_string(),
            }),
            health_path: None,
            properties: HashMap::new(),
        }
    }

    /// GCP Secret Manager preset
    pub fn gcp_secret_manager(project_id: &str) -> AdapterConfig {
        AdapterConfig {
            id: format!("gcp-secrets-{}", project_id),
            adapter_type: AdapterType::GcpSecretManager,
            endpoint: "secretmanager.googleapis.com".to_string(),
            auth: None, // Uses default credentials
            health_path: None,
            properties: [("project_id".to_string(), project_id.to_string())]
                .into_iter()
                .collect(),
        }
    }

    /// HashiCorp Vault preset
    pub fn hashicorp_vault(addr: &str) -> AdapterConfig {
        AdapterConfig {
            id: "hashicorp-vault".to_string(),
            adapter_type: AdapterType::HashicorpVault,
            endpoint: addr.to_string(),
            auth: None,
            health_path: Some("/v1/sys/health".to_string()),
            properties: HashMap::new(),
        }
    }

    /// Redis preset
    pub fn redis(host: &str, port: u16) -> AdapterConfig {
        AdapterConfig {
            id: format!("redis-{}", host),
            adapter_type: AdapterType::Redis,
            endpoint: format!("{}:{}", host, port),
            auth: None,
            health_path: None,
            properties: HashMap::new(),
        }
    }

    /// PostgreSQL preset
    pub fn postgres(host: &str, port: u16, database: &str) -> AdapterConfig {
        AdapterConfig {
            id: format!("postgres-{}-{}", host, database),
            adapter_type: AdapterType::Postgres,
            endpoint: format!("{}:{}", host, port),
            auth: None,
            health_path: None,
            properties: [("database".to_string(), database.to_string())]
                .into_iter()
                .collect(),
        }
    }
}
