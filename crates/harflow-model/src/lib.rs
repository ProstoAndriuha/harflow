use chrono::Utc;
use harflow_core::SCHEMA_VERSION;
use serde::{Deserialize, Serialize};

/// Canonical versioned intermediate API model emitted by analyzers and read by generators.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiModel {
    pub schema_version: String,
    pub project_id: String,
    pub sources: Vec<SessionSource>,
    pub endpoints: Vec<Endpoint>,
    pub schemas: Vec<SchemaDefinition>,
    pub dependencies: Vec<Dependency>,
    pub auth_flows: Vec<AuthFlow>,
    pub workflows: Vec<Workflow>,
    pub diagnostics: Vec<Diagnostic>,
    pub metadata: ModelMetadata,
}

impl ApiModel {
    /// Creates an empty valid `ApiModel` using the canonical schema version.
    #[must_use]
    pub fn empty(project_id: impl Into<String>, project_name: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            project_id: project_id.into(),
            sources: Vec::new(),
            endpoints: Vec::new(),
            schemas: Vec::new(),
            dependencies: Vec::new(),
            auth_flows: Vec::new(),
            workflows: Vec::new(),
            diagnostics: Vec::new(),
            metadata: ModelMetadata {
                project_name: project_name.into(),
                generated_at: Utc::now().to_rfc3339(),
                generator_version: env!("CARGO_PKG_VERSION").to_string(),
                total_requests: 0,
                excluded_requests: 0,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSource {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub har_version: Option<String>,
    pub imported_at: String,
    pub entry_count: usize,
    pub redacted_secrets_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterLocation {
    Path,
    Query,
    Header,
    Cookie,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointParameter {
    pub name: String,
    pub location: ParameterLocation,
    pub inferred_type: String,
    pub required: bool,
    pub observed_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    pub method: String,
    pub host: String,
    pub path_template: String,
    pub parameters: Vec<EndpointParameter>,
    pub request_schema_id: Option<String>,
    pub response_schema_id: Option<String>,
    pub status_codes: Vec<u16>,
    pub confidence: f64,
    pub category: String,
    pub observed_request_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaField {
    pub name: String,
    pub inferred_type: String,
    pub required: bool,
    pub nullable: bool,
    pub observed_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaDefinition {
    pub id: String,
    pub name: String,
    pub root_type: String,
    pub fields: Vec<SchemaField>,
    pub observation_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueLocation {
    Path,
    Query,
    Header,
    Cookie,
    Body,
    SetCookie,
    ResponseHeader,
    ResponseBody,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueReference {
    pub session_id: String,
    pub request_id: String,
    pub location: ValueLocation,
    pub json_path_or_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyType {
    ResponseToHeader,
    ResponseToQuery,
    ResponseToBody,
    ResponseToPath,
    SetCookieToCookie,
    PaginationChain,
    SessionChain,
    DerivedValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transformation {
    Identity,
    UrlEncoding,
    Base64,
    PrefixSuffix,
    JsonExtract,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationStatus {
    Confirmed,
    Likely,
    Unknown,
    Rejected,
    UserConfirmed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub session_id: String,
    pub request_id: String,
    pub rule_id: String,
    pub description: String,
    pub observed_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub id: String,
    pub source: ValueReference,
    pub target: ValueReference,
    #[serde(rename = "type")]
    pub dependency_type: DependencyType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transformation: Option<Transformation>,
    pub confidence: f64,
    pub status: ConfirmationStatus,
    pub evidence: Vec<Evidence>,
    pub observed_session_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthScheme {
    Bearer,
    CookieSession,
    Csrf,
    OauthLike,
    ApiKey,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthFlow {
    pub id: String,
    pub scheme: AuthScheme,
    pub description: String,
    pub request_ids: Vec<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub step_endpoint_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub related_entity_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelMetadata {
    pub project_name: String,
    pub generated_at: String,
    pub generator_version: String,
    pub total_requests: usize,
    #[serde(default)]
    pub excluded_requests: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_empty_api_model_in_camel_case() {
        let model = ApiModel::empty("proj-1", "Demo API");
        let json = serde_json::to_value(&model).expect("serialize model");
        assert_eq!(json["schemaVersion"], SCHEMA_VERSION);
        assert_eq!(json["projectId"], "proj-1");
        assert_eq!(json["metadata"]["projectName"], "Demo API");
        let roundtrip: ApiModel = serde_json::from_value(json).expect("deserialize model");
        assert_eq!(roundtrip.project_id, "proj-1");
    }

    #[test]
    fn deserializes_minimal_api_model_fixture() {
        let fixture_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates")
            .parent()
            .expect("repo root")
            .join("fixtures/expected-output/minimal-api-model.json");
        let raw = std::fs::read_to_string(fixture_path).expect("read fixture");
        let model: ApiModel = serde_json::from_str(&raw).expect("deserialize fixture");
        assert_eq!(model.schema_version, SCHEMA_VERSION);
        assert_eq!(model.endpoints.len(), 1);
        assert_eq!(model.dependencies.len(), 1);
        assert_eq!(model.dependencies[0].status, ConfirmationStatus::Likely);
    }
}
