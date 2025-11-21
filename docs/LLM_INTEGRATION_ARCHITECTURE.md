# LLM-Specific Integration Architecture

**Version:** 1.0.0
**Date:** 2025-11-21
**Author:** System Architect Agent
**Status:** Complete

---

## Executive Summary

This document specifies the LLM-specific features and integration patterns for the LLM-Config-Manager. It covers model endpoint configuration, prompt template versioning, API parameter management, and integration with the LLM DevOps ecosystem.

---

## Table of Contents

1. [Model Endpoint Configuration](#1-model-endpoint-configuration)
2. [Prompt Template Management](#2-prompt-template-management)
3. [API Parameter Configuration](#3-api-parameter-configuration)
4. [Multi-Provider Fallback Chains](#4-multi-provider-fallback-chains)
5. [Cost Tracking and Optimization](#5-cost-tracking-and-optimization)
6. [Integration with LLM-Auto-Optimizer](#6-integration-with-llm-auto-optimizer)

---

## 1. Model Endpoint Configuration

### 1.1 Model Endpoint Schema

```rust
/// LLM model endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ModelEndpoint {
    /// Unique identifier
    pub id: Uuid,

    /// Human-readable name
    #[validate(length(min = 1, max = 100))]
    pub name: String,

    /// Provider type
    pub provider: LlmProvider,

    /// Model identifier
    /// Examples: "gpt-4-turbo", "claude-3-opus-20240229"
    #[validate(length(min = 1, max = 200))]
    pub model_id: String,

    /// API endpoint URL
    #[validate(url)]
    pub endpoint_url: String,

    /// API version
    pub api_version: String,

    /// Cloud region (for AWS Bedrock, Azure OpenAI, GCP Vertex)
    pub region: Option<String>,

    /// Authentication configuration
    pub auth_config: AuthConfig,

    /// Fallback chain for high availability
    pub fallback_endpoints: Vec<ModelEndpoint>,

    /// Performance configuration
    pub performance: PerformanceConfig,

    /// Cost tracking
    pub cost_config: CostConfig,

    /// Metadata
    pub metadata: ModelMetadata,
}

/// LLM provider enumeration
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LlmProvider {
    OpenAI,
    Anthropic,
    AWSBedrock,
    AzureOpenAI,
    GCPVertex,
    Cohere,
    HuggingFace,
    Custom(String),
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AuthConfig {
    /// API key authentication
    ApiKey {
        /// Secret reference (stored encrypted)
        secret_ref: ConfigReference,
    },

    /// OAuth 2.0
    OAuth2 {
        client_id: String,
        client_secret_ref: ConfigReference,
        token_url: String,
        scopes: Vec<String>,
    },

    /// AWS IAM (for Bedrock)
    AwsIam {
        access_key_id_ref: ConfigReference,
        secret_access_key_ref: ConfigReference,
        session_token_ref: Option<ConfigReference>,
    },

    /// Azure Active Directory
    AzureAd {
        tenant_id: String,
        client_id: String,
        client_secret_ref: ConfigReference,
    },

    /// GCP Service Account
    GcpServiceAccount {
        service_account_key_ref: ConfigReference,
    },
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct PerformanceConfig {
    /// Request timeout
    #[validate(range(min = 1, max = 300))]
    pub timeout_seconds: u32,

    /// Maximum retries on failure
    #[validate(range(min = 0, max = 10))]
    pub max_retries: u32,

    /// Retry backoff strategy
    pub retry_backoff: BackoffStrategy,

    /// Connection pool size
    #[validate(range(min = 1, max = 100))]
    pub connection_pool_size: u32,

    /// Keep-alive timeout
    #[validate(range(min = 10, max = 300))]
    pub keep_alive_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackoffStrategy {
    /// Fixed delay between retries
    Fixed { delay_ms: u64 },

    /// Exponential backoff
    Exponential {
        initial_delay_ms: u64,
        max_delay_ms: u64,
        multiplier: f64,
    },

    /// Linear backoff
    Linear {
        initial_delay_ms: u64,
        increment_ms: u64,
    },
}

/// Model metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    /// Context window size (tokens)
    pub context_window: u32,

    /// Supported modalities
    pub modalities: Vec<Modality>,

    /// Streaming support
    pub supports_streaming: bool,

    /// Function calling support
    pub supports_function_calling: bool,

    /// JSON mode support
    pub supports_json_mode: bool,

    /// Vision support
    pub supports_vision: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Modality {
    Text,
    Image,
    Audio,
    Video,
    Code,
}

/// Cost tracking configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostConfig {
    /// Cost per 1K input tokens (USD)
    pub cost_per_1k_input_tokens: f64,

    /// Cost per 1K output tokens (USD)
    pub cost_per_1k_output_tokens: f64,

    /// Cost per image (if applicable)
    pub cost_per_image: Option<f64>,

    /// Monthly budget limit (USD)
    pub monthly_budget_limit: Option<f64>,

    /// Alert threshold (percentage of budget)
    pub alert_threshold: Option<f64>,
}
```

### 1.2 Example Model Endpoint Configurations

```yaml
# OpenAI GPT-4 Turbo
model_endpoints:
  gpt4_turbo_prod:
    name: "GPT-4 Turbo Production"
    provider: "OpenAI"
    model_id: "gpt-4-turbo-preview"
    endpoint_url: "https://api.openai.com/v1"
    api_version: "2024-01-01"
    auth_config:
      type: "ApiKey"
      secret_ref:
        namespace: "production/ml-platform/secrets"
        key: "openai-api-key"
    performance:
      timeout_seconds: 60
      max_retries: 3
      retry_backoff:
        type: "Exponential"
        initial_delay_ms: 1000
        max_delay_ms: 30000
        multiplier: 2.0
      connection_pool_size: 10
      keep_alive_seconds: 60
    cost_config:
      cost_per_1k_input_tokens: 0.01
      cost_per_1k_output_tokens: 0.03
      monthly_budget_limit: 10000.0
      alert_threshold: 0.8
    metadata:
      context_window: 128000
      modalities: ["Text", "Image", "Code"]
      supports_streaming: true
      supports_function_calling: true
      supports_json_mode: true
      supports_vision: true
    fallback_endpoints:
      - name: "GPT-4 Turbo Backup"
        provider: "AzureOpenAI"
        model_id: "gpt-4-turbo"
        endpoint_url: "https://llm-platform.openai.azure.com"
        region: "eastus"
        # ... rest of config

# Anthropic Claude 3 Opus
  claude3_opus_prod:
    name: "Claude 3 Opus Production"
    provider: "Anthropic"
    model_id: "claude-3-opus-20240229"
    endpoint_url: "https://api.anthropic.com"
    api_version: "2023-06-01"
    auth_config:
      type: "ApiKey"
      secret_ref:
        namespace: "production/ml-platform/secrets"
        key: "anthropic-api-key"
    performance:
      timeout_seconds: 120
      max_retries: 3
      retry_backoff:
        type: "Exponential"
        initial_delay_ms: 1000
        max_delay_ms: 30000
        multiplier: 2.0
      connection_pool_size: 10
      keep_alive_seconds: 60
    cost_config:
      cost_per_1k_input_tokens: 0.015
      cost_per_1k_output_tokens: 0.075
      monthly_budget_limit: 15000.0
      alert_threshold: 0.85
    metadata:
      context_window: 200000
      modalities: ["Text", "Image"]
      supports_streaming: true
      supports_function_calling: false
      supports_json_mode: false
      supports_vision: true

# AWS Bedrock Claude 3 Sonnet
  bedrock_claude3_sonnet:
    name: "Bedrock Claude 3 Sonnet"
    provider: "AWSBedrock"
    model_id: "anthropic.claude-3-sonnet-20240229-v1:0"
    endpoint_url: "https://bedrock-runtime.us-east-1.amazonaws.com"
    api_version: "2023-09-30"
    region: "us-east-1"
    auth_config:
      type: "AwsIam"
      access_key_id_ref:
        namespace: "production/ml-platform/secrets"
        key: "aws-access-key-id"
      secret_access_key_ref:
        namespace: "production/ml-platform/secrets"
        key: "aws-secret-access-key"
    performance:
      timeout_seconds: 90
      max_retries: 3
      retry_backoff:
        type: "Exponential"
        initial_delay_ms: 1000
        max_delay_ms: 30000
        multiplier: 2.0
      connection_pool_size: 15
      keep_alive_seconds: 60
    cost_config:
      cost_per_1k_input_tokens: 0.003
      cost_per_1k_output_tokens: 0.015
      monthly_budget_limit: 8000.0
      alert_threshold: 0.8
    metadata:
      context_window: 200000
      modalities: ["Text", "Image"]
      supports_streaming: true
      supports_function_calling: false
      supports_json_mode: false
      supports_vision: true
```

---

## 2. Prompt Template Management

### 2.1 Prompt Template Schema

```rust
/// Prompt template with versioning
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct PromptTemplate {
    /// Unique identifier
    pub id: Uuid,

    /// Human-readable name
    #[validate(length(min = 1, max = 200))]
    pub name: String,

    /// Semantic version (e.g., "1.2.3")
    #[validate(regex = r"^\d+\.\d+\.\d+$")]
    pub version: String,

    /// Template content (Handlebars-style)
    pub template: String,

    /// Required variables
    pub variables: Vec<TemplateVariable>,

    /// Optional system message
    pub system_message: Option<String>,

    /// Model-specific parameters
    pub model_parameters: ModelParameters,

    /// Metadata
    pub metadata: PromptMetadata,

    /// Version control
    pub git_commit: Option<String>,
    pub parent_version: Option<String>,

    /// Lifecycle
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub updated_at: DateTime<Utc>,
    pub updated_by: String,

    /// Tags for organization
    pub tags: Vec<String>,
}

/// Template variable definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVariable {
    /// Variable name (used in template as {{name}})
    pub name: String,

    /// Type hint for validation
    pub type_hint: VariableType,

    /// Required or optional
    pub required: bool,

    /// Default value
    pub default: Option<String>,

    /// Validation rule
    pub validation: Option<ValidationRule>,

    /// Description
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VariableType {
    String,
    Integer,
    Float,
    Boolean,
    Array,
    Object,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationRule {
    /// Regex pattern
    Pattern(String),

    /// Minimum length
    MinLength(usize),

    /// Maximum length
    MaxLength(usize),

    /// Allowed values
    Enum(Vec<String>),

    /// Numeric range
    Range { min: f64, max: f64 },
}

/// Model parameters for LLM API calls
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ModelParameters {
    /// Temperature (0.0 - 2.0)
    #[validate(range(min = 0.0, max = 2.0))]
    pub temperature: f32,

    /// Top-p sampling (0.0 - 1.0)
    #[validate(range(min = 0.0, max = 1.0))]
    pub top_p: f32,

    /// Maximum tokens to generate
    #[validate(range(min = 1, max = 128000))]
    pub max_tokens: u32,

    /// Top-k sampling (optional)
    pub top_k: Option<u32>,

    /// Frequency penalty (-2.0 to 2.0)
    #[validate(range(min = -2.0, max = 2.0))]
    pub frequency_penalty: Option<f32>,

    /// Presence penalty (-2.0 to 2.0)
    #[validate(range(min = -2.0, max = 2.0))]
    pub presence_penalty: Option<f32>,

    /// Stop sequences
    pub stop_sequences: Vec<String>,
}

/// Prompt metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMetadata {
    /// Use case description
    pub use_case: String,

    /// Target model(s)
    pub target_models: Vec<String>,

    /// Performance metrics
    pub performance_metrics: Option<PerformanceMetrics>,

    /// A/B test variant (if applicable)
    pub ab_test_variant: Option<String>,

    /// Owner team
    pub owner_team: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Average latency (ms)
    pub avg_latency_ms: f64,

    /// Average cost per request (USD)
    pub avg_cost_usd: f64,

    /// Success rate (0.0 - 1.0)
    pub success_rate: f64,

    /// User satisfaction score (0.0 - 5.0)
    pub user_satisfaction: Option<f64>,
}
```

### 2.2 Example Prompt Templates

```yaml
# Customer support assistant
prompt_templates:
  customer_support_v1:
    name: "Customer Support Assistant"
    version: "1.0.0"
    template: |
      You are a {{role}} assistant for {{company_name}}.

      Context: {{context}}

      Customer question: {{question}}

      Please provide a {{response_style}} response that:
      - Addresses the customer's question directly
      - Is professional and empathetic
      - Includes relevant links to documentation when applicable
      - Offers to escalate to a human agent if needed

      Response:
    variables:
      - name: "role"
        type_hint: "String"
        required: true
        default: "helpful"
        validation:
          type: "Enum"
          values: ["helpful", "technical", "sales", "billing"]
        description: "Role of the assistant"
      - name: "company_name"
        type_hint: "String"
        required: true
        description: "Name of the company"
      - name: "context"
        type_hint: "String"
        required: false
        default: ""
        description: "Additional context about the customer or issue"
      - name: "question"
        type_hint: "String"
        required: true
        validation:
          type: "MinLength"
          value: 10
        description: "Customer's question"
      - name: "response_style"
        type_hint: "String"
        required: true
        default: "concise and friendly"
        description: "Desired response style"
    system_message: |
      You are an AI customer support assistant. Always be polite,
      professional, and accurate. If you don't know the answer,
      admit it and offer to escalate to a human agent.
    model_parameters:
      temperature: 0.7
      top_p: 0.9
      max_tokens: 500
      frequency_penalty: 0.0
      presence_penalty: 0.0
      stop_sequences: []
    metadata:
      use_case: "Customer support chat"
      target_models: ["gpt-4-turbo", "claude-3-opus"]
      owner_team: "customer-experience"
      ab_test_variant: "A"
    tags: ["customer-support", "production", "chat"]

# Code generation assistant
  code_generation_v2:
    name: "Code Generation Assistant"
    version: "2.1.0"
    template: |
      You are an expert {{language}} programmer.

      Task: {{task}}

      Requirements:
      {{#each requirements}}
      - {{this}}
      {{/each}}

      {{#if existing_code}}
      Existing code:
      ```{{language}}
      {{existing_code}}
      ```
      {{/if}}

      Please provide {{output_format}} code that:
      - Follows {{language}} best practices
      - Includes error handling
      - Has clear comments
      - Is production-ready

      {{#if tests_required}}
      Also include unit tests.
      {{/if}}

      Code:
    variables:
      - name: "language"
        type_hint: "String"
        required: true
        validation:
          type: "Enum"
          values: ["Python", "JavaScript", "TypeScript", "Rust", "Go", "Java"]
        description: "Programming language"
      - name: "task"
        type_hint: "String"
        required: true
        validation:
          type: "MinLength"
          value: 20
        description: "Description of the coding task"
      - name: "requirements"
        type_hint: "Array"
        required: false
        default: []
        description: "List of requirements"
      - name: "existing_code"
        type_hint: "String"
        required: false
        description: "Existing code to modify or extend"
      - name: "output_format"
        type_hint: "String"
        required: true
        default: "clean, well-structured"
        description: "Desired output format"
      - name: "tests_required"
        type_hint: "Boolean"
        required: false
        default: false
        description: "Whether to include unit tests"
    system_message: |
      You are an expert programmer who writes clean, efficient,
      and well-documented code. Always follow best practices and
      industry standards.
    model_parameters:
      temperature: 0.3
      top_p: 0.95
      max_tokens: 2000
      frequency_penalty: 0.0
      presence_penalty: 0.0
      stop_sequences: ["```\n\n"]
    metadata:
      use_case: "Code generation and refactoring"
      target_models: ["gpt-4-turbo", "claude-3-opus"]
      owner_team: "engineering-productivity"
      performance_metrics:
        avg_latency_ms: 3500
        avg_cost_usd: 0.15
        success_rate: 0.92
        user_satisfaction: 4.3
    git_commit: "abc123def456"
    parent_version: "2.0.1"
    tags: ["code-generation", "production", "engineering"]
```

### 2.3 Prompt Template Rendering

```rust
use handlebars::Handlebars;

/// Prompt template renderer
pub struct PromptRenderer {
    handlebars: Handlebars<'static>,
}

impl PromptRenderer {
    pub fn new() -> Self {
        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);

        Self { handlebars }
    }

    /// Render prompt template with variables
    pub fn render(
        &self,
        template: &PromptTemplate,
        variables: &HashMap<String, serde_json::Value>,
    ) -> Result<String> {
        // Validate required variables
        for var in &template.variables {
            if var.required && !variables.contains_key(&var.name) {
                return Err(Error::MissingRequiredVariable {
                    variable: var.name.clone(),
                });
            }

            // Apply validation rules
            if let Some(value) = variables.get(&var.name) {
                self.validate_variable(var, value)?;
            }
        }

        // Apply defaults for missing optional variables
        let mut final_vars = variables.clone();
        for var in &template.variables {
            if !var.required && !final_vars.contains_key(&var.name) {
                if let Some(default) = &var.default {
                    final_vars.insert(
                        var.name.clone(),
                        serde_json::Value::String(default.clone()),
                    );
                }
            }
        }

        // Render template
        let rendered = self.handlebars.render_template(
            &template.template,
            &final_vars,
        )?;

        Ok(rendered)
    }

    /// Validate variable against rules
    fn validate_variable(
        &self,
        var: &TemplateVariable,
        value: &serde_json::Value,
    ) -> Result<()> {
        if let Some(rule) = &var.validation {
            match rule {
                ValidationRule::Pattern(pattern) => {
                    let regex = Regex::new(pattern)?;
                    let str_value = value.as_str()
                        .ok_or(Error::InvalidVariableType)?;
                    if !regex.is_match(str_value) {
                        return Err(Error::ValidationFailed {
                            variable: var.name.clone(),
                            rule: "pattern",
                        });
                    }
                }
                ValidationRule::MinLength(min) => {
                    let str_value = value.as_str()
                        .ok_or(Error::InvalidVariableType)?;
                    if str_value.len() < *min {
                        return Err(Error::ValidationFailed {
                            variable: var.name.clone(),
                            rule: "min_length",
                        });
                    }
                }
                ValidationRule::MaxLength(max) => {
                    let str_value = value.as_str()
                        .ok_or(Error::InvalidVariableType)?;
                    if str_value.len() > *max {
                        return Err(Error::ValidationFailed {
                            variable: var.name.clone(),
                            rule: "max_length",
                        });
                    }
                }
                ValidationRule::Enum(allowed) => {
                    let str_value = value.as_str()
                        .ok_or(Error::InvalidVariableType)?;
                    if !allowed.contains(&str_value.to_string()) {
                        return Err(Error::ValidationFailed {
                            variable: var.name.clone(),
                            rule: "enum",
                        });
                    }
                }
                ValidationRule::Range { min, max } => {
                    let num_value = value.as_f64()
                        .ok_or(Error::InvalidVariableType)?;
                    if num_value < *min || num_value > *max {
                        return Err(Error::ValidationFailed {
                            variable: var.name.clone(),
                            rule: "range",
                        });
                    }
                }
            }
        }

        Ok(())
    }
}
```

---

## 3. API Parameter Configuration

### 3.1 Parameter Presets

```yaml
# Development environment - higher creativity
parameter_presets:
  development_creative:
    environment: "development"
    temperature: 0.9
    top_p: 0.95
    max_tokens: 2000
    frequency_penalty: 0.3
    presence_penalty: 0.3
    stop_sequences: []
    use_case: "Exploratory development, brainstorming"

  # Production environment - more deterministic
  production_deterministic:
    environment: "production"
    temperature: 0.3
    top_p: 0.9
    max_tokens: 1000
    frequency_penalty: 0.0
    presence_penalty: 0.0
    stop_sequences: ["\n\n---\n\n"]
    use_case: "Production customer-facing applications"

  # Code generation - balanced
  code_generation:
    temperature: 0.4
    top_p: 0.95
    max_tokens: 2500
    frequency_penalty: 0.0
    presence_penalty: 0.0
    stop_sequences: ["```\n\n", "# End of code"]
    use_case: "Code generation and refactoring"

  # Customer support - empathetic
  customer_support:
    temperature: 0.7
    top_p: 0.9
    max_tokens: 500
    frequency_penalty: 0.2
    presence_penalty: 0.1
    stop_sequences: []
    use_case: "Customer support chat"

  # Data extraction - precise
  data_extraction:
    temperature: 0.1
    top_p: 0.85
    max_tokens: 1000
    frequency_penalty: 0.0
    presence_penalty: 0.0
    stop_sequences: []
    use_case: "Structured data extraction"
```

---

## 4. Multi-Provider Fallback Chains

### 4.1 Failover Architecture

```rust
/// Multi-provider failover manager
pub struct FailoverManager {
    primary_endpoint: Arc<ModelEndpoint>,
    fallback_chain: Vec<Arc<ModelEndpoint>>,
    circuit_breaker: Arc<CircuitBreaker>,
}

impl FailoverManager {
    /// Execute LLM request with automatic failover
    pub async fn execute_with_failover(
        &self,
        request: LlmRequest,
    ) -> Result<LlmResponse> {
        let mut errors = Vec::new();

        // Try primary endpoint
        match self.try_endpoint(&self.primary_endpoint, &request).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                tracing::warn!(
                    endpoint = %self.primary_endpoint.name,
                    error = %e,
                    "Primary endpoint failed, trying fallbacks"
                );
                errors.push((self.primary_endpoint.name.clone(), e));
            }
        }

        // Try fallback endpoints
        for fallback in &self.fallback_chain {
            if self.circuit_breaker.is_open(&fallback.id).await {
                tracing::info!(
                    endpoint = %fallback.name,
                    "Circuit breaker open, skipping endpoint"
                );
                continue;
            }

            match self.try_endpoint(fallback, &request).await {
                Ok(response) => {
                    tracing::info!(
                        endpoint = %fallback.name,
                        "Failover successful"
                    );
                    return Ok(response);
                }
                Err(e) => {
                    tracing::warn!(
                        endpoint = %fallback.name,
                        error = %e,
                        "Fallback endpoint failed"
                    );
                    errors.push((fallback.name.clone(), e));

                    // Open circuit breaker on repeated failures
                    self.circuit_breaker.record_failure(&fallback.id).await;
                }
            }
        }

        // All endpoints failed
        Err(Error::AllEndpointsFailed { errors })
    }

    /// Try a single endpoint
    async fn try_endpoint(
        &self,
        endpoint: &ModelEndpoint,
        request: &LlmRequest,
    ) -> Result<LlmResponse> {
        let client = self.get_client(endpoint)?;

        // Apply retry logic
        let mut retries = 0;
        loop {
            match client.send_request(request).await {
                Ok(response) => return Ok(response),
                Err(e) if retries < endpoint.performance.max_retries => {
                    retries += 1;
                    let delay = self.calculate_backoff(
                        &endpoint.performance.retry_backoff,
                        retries,
                    );

                    tracing::debug!(
                        endpoint = %endpoint.name,
                        retry = retries,
                        delay_ms = delay.as_millis(),
                        "Retrying request"
                    );

                    tokio::time::sleep(delay).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Calculate retry backoff delay
    fn calculate_backoff(
        &self,
        strategy: &BackoffStrategy,
        retry_count: u32,
    ) -> Duration {
        match strategy {
            BackoffStrategy::Fixed { delay_ms } => {
                Duration::from_millis(*delay_ms)
            }
            BackoffStrategy::Exponential {
                initial_delay_ms,
                max_delay_ms,
                multiplier,
            } => {
                let delay = (*initial_delay_ms as f64)
                    * multiplier.powi(retry_count as i32);
                let delay = delay.min(*max_delay_ms as f64);
                Duration::from_millis(delay as u64)
            }
            BackoffStrategy::Linear {
                initial_delay_ms,
                increment_ms,
            } => {
                let delay = initial_delay_ms
                    + (increment_ms * retry_count as u64);
                Duration::from_millis(delay)
            }
        }
    }
}

/// Circuit breaker for endpoint health
pub struct CircuitBreaker {
    states: Arc<RwLock<HashMap<Uuid, CircuitBreakerState>>>,
    failure_threshold: u32,
    recovery_timeout: Duration,
}

#[derive(Debug, Clone)]
struct CircuitBreakerState {
    failures: u32,
    last_failure: Option<Instant>,
    state: CircuitState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircuitState {
    Closed,     // Normal operation
    Open,       // Failing, reject requests
    HalfOpen,   // Testing recovery
}

impl CircuitBreaker {
    pub async fn is_open(&self, endpoint_id: &Uuid) -> bool {
        let states = self.states.read().await;
        if let Some(state) = states.get(endpoint_id) {
            matches!(state.state, CircuitState::Open)
        } else {
            false
        }
    }

    pub async fn record_failure(&self, endpoint_id: &Uuid) {
        let mut states = self.states.write().await;
        let state = states.entry(*endpoint_id)
            .or_insert_with(|| CircuitBreakerState {
                failures: 0,
                last_failure: None,
                state: CircuitState::Closed,
            });

        state.failures += 1;
        state.last_failure = Some(Instant::now());

        if state.failures >= self.failure_threshold {
            state.state = CircuitState::Open;
            tracing::warn!(
                endpoint_id = %endpoint_id,
                "Circuit breaker opened"
            );
        }
    }

    pub async fn record_success(&self, endpoint_id: &Uuid) {
        let mut states = self.states.write().await;
        if let Some(state) = states.get_mut(endpoint_id) {
            state.failures = 0;
            state.state = CircuitState::Closed;
        }
    }
}
```

---

## 5. Cost Tracking and Optimization

### 5.1 Cost Tracker

```rust
/// Real-time cost tracking
pub struct CostTracker {
    db: Arc<PgPool>,
    redis: Arc<RedisClient>,
}

impl CostTracker {
    /// Record LLM API usage
    pub async fn record_usage(
        &self,
        usage: LlmUsage,
    ) -> Result<()> {
        // Calculate cost
        let cost = self.calculate_cost(&usage)?;

        // Store in database
        sqlx::query!(
            r#"
            INSERT INTO llm_usage (
                id, timestamp, endpoint_id, model_id,
                input_tokens, output_tokens, cost_usd,
                latency_ms, request_id, user_id
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            Uuid::new_v4(),
            usage.timestamp,
            usage.endpoint_id,
            usage.model_id,
            usage.input_tokens as i32,
            usage.output_tokens as i32,
            cost,
            usage.latency_ms as i32,
            usage.request_id,
            usage.user_id,
        )
        .execute(&*self.db)
        .await?;

        // Update real-time counters in Redis
        self.update_redis_counters(&usage, cost).await?;

        // Check budget alerts
        self.check_budget_alerts(&usage.endpoint_id, cost).await?;

        Ok(())
    }

    /// Calculate cost based on token usage
    fn calculate_cost(&self, usage: &LlmUsage) -> Result<f64> {
        let endpoint = self.get_endpoint(&usage.endpoint_id)?;

        let input_cost = (usage.input_tokens as f64 / 1000.0)
            * endpoint.cost_config.cost_per_1k_input_tokens;

        let output_cost = (usage.output_tokens as f64 / 1000.0)
            * endpoint.cost_config.cost_per_1k_output_tokens;

        Ok(input_cost + output_cost)
    }

    /// Update Redis counters for real-time dashboards
    async fn update_redis_counters(
        &self,
        usage: &LlmUsage,
        cost: f64,
    ) -> Result<()> {
        let key_prefix = format!(
            "llm_usage:{}:{}",
            usage.endpoint_id,
            Utc::now().format("%Y-%m")
        );

        // Increment monthly token counters
        self.redis.incr(
            &format!("{}:input_tokens", key_prefix),
            usage.input_tokens,
        ).await?;

        self.redis.incr(
            &format!("{}:output_tokens", key_prefix),
            usage.output_tokens,
        ).await?;

        // Increment monthly cost
        self.redis.incr_by_float(
            &format!("{}:cost_usd", key_prefix),
            cost,
        ).await?;

        // Increment request count
        self.redis.incr(
            &format!("{}:requests", key_prefix),
            1,
        ).await?;

        Ok(())
    }

    /// Check if budget alerts should be triggered
    async fn check_budget_alerts(
        &self,
        endpoint_id: &Uuid,
        new_cost: f64,
    ) -> Result<()> {
        let endpoint = self.get_endpoint(endpoint_id)?;

        if let Some(budget_limit) = endpoint.cost_config.monthly_budget_limit {
            let current_month_cost = self.get_monthly_cost(endpoint_id).await?;

            if let Some(threshold) = endpoint.cost_config.alert_threshold {
                let threshold_amount = budget_limit * threshold;

                if current_month_cost >= threshold_amount
                    && current_month_cost - new_cost < threshold_amount
                {
                    // Threshold crossed, send alert
                    self.send_budget_alert(
                        endpoint_id,
                        current_month_cost,
                        budget_limit,
                        threshold,
                    ).await?;
                }
            }

            if current_month_cost >= budget_limit {
                // Budget exceeded, send critical alert
                self.send_budget_exceeded_alert(
                    endpoint_id,
                    current_month_cost,
                    budget_limit,
                ).await?;
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct LlmUsage {
    pub timestamp: DateTime<Utc>,
    pub endpoint_id: Uuid,
    pub model_id: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub latency_ms: u64,
    pub request_id: String,
    pub user_id: Option<String>,
}
```

---

## 6. Integration with LLM-Auto-Optimizer

### 6.1 Optimization Proposal Schema

```rust
/// Configuration change proposal from Auto-Optimizer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationProposal {
    pub id: Uuid,
    pub proposal_type: ProposalType,
    pub namespace: String,
    pub key: String,
    pub current_value: ConfigValue,
    pub proposed_value: ConfigValue,
    pub justification: String,
    pub expected_impact: ImpactEstimate,
    pub approval_required: bool,
    pub auto_approve_rules: Vec<AutoApproveRule>,
    pub rollback_config: RollbackConfig,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposalType {
    ParameterTuning,        // Adjust model parameters
    ModelSwitch,            // Switch to different model
    EndpointChange,         // Change API endpoint
    CostOptimization,       // Reduce costs
    PerformanceOptimization, // Improve latency/throughput
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactEstimate {
    pub estimated_latency_change: f64,    // -10% = 10% improvement
    pub estimated_throughput_change: f64,
    pub estimated_cost_change: f64,
    pub confidence: f32,                  // 0.0 - 1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoApproveRule {
    pub condition: String,  // CEL expression
    pub max_impact: f64,    // Maximum allowed impact
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackConfig {
    pub auto_rollback: bool,
    pub rollback_threshold: RollbackThreshold,
    pub monitoring_window: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RollbackThreshold {
    ErrorRateIncrease { threshold: f64 },
    LatencyIncrease { threshold_ms: u64 },
    CostIncrease { threshold_usd: f64 },
}
```

### 6.2 Optimization Workflow

```
┌──────────────────────────────────────────────────────────┐
│     LLM-Auto-Optimizer Integration Workflow              │
└──────────────────────────────────────────────────────────┘

1. Performance Monitoring:
   LLM-Auto-Optimizer
       ↓
   Monitor LLM metrics (latency, cost, quality)
       ↓
   Detect suboptimal configuration
       ↓
   Generate OptimizationProposal


2. Proposal Submission:
   OptimizationProposal
       ↓
   Config-Manager receives proposal
       ↓
   Validate against policies (Policy Engine)
       ↓
   Check auto-approve rules


3. Approval Flow:
   If auto-approve rules match:
       ↓
   Apply immediately (canary deployment)
       ↓
   Monitor impact for monitoring_window
       ↓
   If positive: Promote to 100%
   If negative: Auto-rollback

   If manual approval required:
       ↓
   Create change request
       ↓
   Notify approvers (Governance Dashboard)
       ↓
   Await approval
       ↓
   Apply on approval


4. Impact Monitoring:
   Config-Manager
       ↓
   Publish change event
       ↓
   LLM-Auto-Optimizer monitors metrics
       ↓
   Compare actual vs. estimated impact
       ↓
   Report results to Governance Dashboard
```

---

## Conclusion

This LLM-specific integration architecture provides:

1. **Model Endpoint Management**: Multi-provider support with failover
2. **Prompt Versioning**: Git-integrated template management
3. **Parameter Optimization**: Environment-specific presets
4. **Cost Control**: Real-time tracking with budget alerts
5. **Auto-Optimization**: Integration with LLM-Auto-Optimizer for continuous improvement

All LLM-specific configurations are stored securely in the Config-Manager with full audit trails and policy enforcement.

---

**Document Metadata:**
- **Total Schemas:** 15+ Rust type definitions
- **Example Configurations:** 10+ YAML examples
- **Integration Patterns:** 4 major workflows
- **Status:** COMPLETE
