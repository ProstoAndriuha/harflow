use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use harflow_core::{HarflowError, SCHEMA_VERSION};
use harflow_endpoints::discover_endpoints;
use harflow_graph::{build_endpoint_graph, GraphViewModel};
use harflow_har::{parse_har_bytes, ImportLimits};
use harflow_model::{ApiModel, ConfirmationStatus, Dependency, Endpoint, SessionSource};
use harflow_normalizer::{normalize_har_session, NormalizedSession};
use harflow_sessions::detect_observed_auth;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSettings {
    pub retain_original_har: bool,
    pub max_har_size_bytes: usize,
    pub offline_only: bool,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            retain_original_har: false,
            max_har_size_bytes: ImportLimits::default().default_max_bytes,
            offline_only: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStatistics {
    pub total_requests: usize,
    pub unique_endpoints: usize,
    pub api_domains: usize,
    pub dependencies: usize,
    pub auth_flows: usize,
    pub unknown_mappings: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExportMetadata {
    pub last_exported_at: Option<String>,
    pub last_formats: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectManifest {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub sources: Vec<SessionSource>,
    pub settings: ProjectSettings,
    pub statistics: ProjectStatistics,
    pub generator_versions: BTreeMap<String, String>,
    pub export_metadata: ExportMetadata,
    #[serde(default)]
    pub user_overrides: BTreeMap<String, ConfirmationStatus>,
}

/// Manages a portable local project directory on disk without keeping unredacted HAR inputs.
#[derive(Debug, Clone)]
pub struct ProjectWorkspace {
    pub root_dir: PathBuf,
    pub manifest: ProjectManifest,
    pub model: ApiModel,
    pub sessions: Vec<NormalizedSession>,
}

impl ProjectWorkspace {
    /// Creates and persists a new empty project directory atomically.
    pub fn create(
        root_dir: impl AsRef<Path>,
        name: impl Into<String>,
    ) -> Result<Self, HarflowError> {
        let root = root_dir.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;

        let id = Uuid::new_v4().to_string();
        let project_name = name.into();
        let now = Utc::now().to_rfc3339();

        let manifest = ProjectManifest {
            schema_version: SCHEMA_VERSION.to_string(),
            id: id.clone(),
            name: project_name.clone(),
            created_at: now.clone(),
            updated_at: now,
            sources: Vec::new(),
            settings: ProjectSettings::default(),
            statistics: ProjectStatistics::default(),
            generator_versions: BTreeMap::from([(
                "core".to_string(),
                env!("CARGO_PKG_VERSION").to_string(),
            )]),
            export_metadata: ExportMetadata::default(),
            user_overrides: BTreeMap::new(),
        };

        let model = ApiModel::empty(id, project_name);
        let workspace = Self {
            root_dir: root,
            manifest,
            model,
            sessions: Vec::new(),
        };
        workspace.save()?;
        Ok(workspace)
    }

    /// Opens an existing HARFlow project directory.
    pub fn open(root_dir: impl AsRef<Path>) -> Result<Self, HarflowError> {
        let root = root_dir.as_ref().to_path_buf();
        let manifest_path = root.join("project.json");
        let model_path = root.join("api-model.json");
        let sessions_path = root.join("sessions.json");

        let manifest_raw = fs::read_to_string(&manifest_path)?;
        let manifest: ProjectManifest = serde_json::from_str(&manifest_raw)
            .map_err(|err| HarflowError::InvalidSource(format!("invalid project.json: {err}")))?;

        let model_raw = fs::read_to_string(&model_path)?;
        let model: ApiModel = serde_json::from_str(&model_raw)
            .map_err(|err| HarflowError::InvalidSource(format!("invalid api-model.json: {err}")))?;

        let sessions = if sessions_path.exists() {
            let sessions_raw = fs::read_to_string(&sessions_path)?;
            serde_json::from_str(&sessions_raw).map_err(|err| {
                HarflowError::InvalidSource(format!("invalid sessions.json: {err}"))
            })?
        } else {
            Vec::new()
        };

        Ok(Self {
            root_dir: root,
            manifest,
            model,
            sessions,
        })
    }

    /// Saves `project.json`, `api-model.json`, and sanitized `sessions.json` using atomic temp-rename writes.
    pub fn save(&self) -> Result<(), HarflowError> {
        fs::create_dir_all(&self.root_dir)?;
        write_json_atomic(&self.root_dir.join("project.json"), &self.manifest)?;
        write_json_atomic(&self.root_dir.join("api-model.json"), &self.model)?;
        write_json_atomic(&self.root_dir.join("sessions.json"), &self.sessions)?;
        Ok(())
    }

    /// Imports and sanitizes a HAR 1.2 payload without retaining raw credentials.
    pub fn import_har_bytes(
        &mut self,
        source_name: &str,
        bytes: &[u8],
    ) -> Result<SessionSource, HarflowError> {
        let limits = ImportLimits {
            default_max_bytes: self.manifest.settings.max_har_size_bytes,
            ..ImportLimits::default()
        };
        let har = parse_har_bytes(bytes, &limits)?;
        let session_id = format!("sess-{:04}", self.sessions.len() + 1);
        let normalized = normalize_har_session(&session_id, source_name, &har);

        let source = SessionSource {
            id: session_id,
            name: source_name.to_string(),
            har_version: Some(normalized.har_version.clone()),
            imported_at: Utc::now().to_rfc3339(),
            entry_count: normalized.entries.len(),
            redacted_secrets_count: normalized.redacted_secrets_count,
        };

        self.sessions.push(normalized);
        self.manifest.sources.push(source.clone());
        self.model.sources.push(source.clone());
        self.refresh_statistics();
        self.manifest.updated_at = Utc::now().to_rfc3339();
        self.save()?;

        Ok(source)
    }

    /// Runs deterministic endpoint discovery and session auth detection while preserving user overrides.
    pub fn analyze(&mut self) -> Result<(), HarflowError> {
        self.model.endpoints = discover_endpoints(&self.sessions);
        self.model.auth_flows = detect_observed_auth(&self.sessions);

        for dep in &mut self.model.dependencies {
            if let Some(status) = self.manifest.user_overrides.get(&dep.id) {
                dep.status = *status;
            }
        }

        self.model.metadata.generated_at = Utc::now().to_rfc3339();
        self.refresh_statistics();
        self.manifest.updated_at = Utc::now().to_rfc3339();
        self.save()?;
        Ok(())
    }

    /// Confirms or rejects a dependency and persists the user override.
    pub fn set_dependency_status(
        &mut self,
        dependency_id: &str,
        status: ConfirmationStatus,
    ) -> Result<(), HarflowError> {
        let dep = self
            .model
            .dependencies
            .iter_mut()
            .find(|d| d.id == dependency_id)
            .ok_or_else(|| {
                HarflowError::InvalidArgs(format!("dependency '{dependency_id}' not found"))
            })?;

        dep.status = status;
        self.manifest
            .user_overrides
            .insert(dependency_id.to_string(), status);
        self.refresh_statistics();
        self.manifest.updated_at = Utc::now().to_rfc3339();
        self.save()?;
        Ok(())
    }

    fn refresh_statistics(&mut self) {
        let total_requests: usize = self.sessions.iter().map(|s| s.entries.len()).sum();
        let domains: BTreeSet<&str> = self
            .model
            .endpoints
            .iter()
            .map(|e| e.host.as_str())
            .collect();
        let unknown_mappings = self
            .model
            .dependencies
            .iter()
            .filter(|d| d.status == ConfirmationStatus::Unknown)
            .count();

        self.model.metadata.total_requests = total_requests;
        self.manifest.statistics = ProjectStatistics {
            total_requests,
            unique_endpoints: self.model.endpoints.len(),
            api_domains: domains.len(),
            dependencies: self.model.dependencies.len(),
            auth_flows: self.model.auth_flows.len(),
            unknown_mappings,
        };
    }
}

/// Typed internal application commands exposed to UI/CLI adapters (SRS Section 5).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", content = "payload")]
pub enum AppCommand {
    #[serde(rename = "project.create")]
    ProjectCreate { root_dir: PathBuf, name: String },
    #[serde(rename = "project.open")]
    ProjectOpen { root_dir: PathBuf },
    #[serde(rename = "project.save")]
    ProjectSave { root_dir: PathBuf },
    #[serde(rename = "har.import")]
    HarImport {
        root_dir: PathBuf,
        source_name: String,
        har_bytes: Vec<u8>,
    },
    #[serde(rename = "analysis.start")]
    AnalysisStart { root_dir: PathBuf },
    #[serde(rename = "endpoints.list")]
    EndpointsList { root_dir: PathBuf },
    #[serde(rename = "dependencies.list")]
    DependenciesList { root_dir: PathBuf },
    #[serde(rename = "dependencies.confirm")]
    DependenciesConfirm {
        root_dir: PathBuf,
        dependency_id: String,
    },
    #[serde(rename = "dependencies.reject")]
    DependenciesReject {
        root_dir: PathBuf,
        dependency_id: String,
    },
    #[serde(rename = "graph.get")]
    GraphGet { root_dir: PathBuf },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum AppResponse {
    Manifest(ProjectManifest),
    Source(SessionSource),
    Model(ApiModel),
    Endpoints(Vec<Endpoint>),
    Dependencies(Vec<Dependency>),
    Graph(GraphViewModel),
}

/// Dispatches an internal typed application command against the local workspace storage.
pub fn dispatch_command(cmd: AppCommand) -> Result<AppResponse, HarflowError> {
    match cmd {
        AppCommand::ProjectCreate { root_dir, name } => {
            let ws = ProjectWorkspace::create(root_dir, name)?;
            Ok(AppResponse::Manifest(ws.manifest))
        }
        AppCommand::ProjectOpen { root_dir } => {
            let ws = ProjectWorkspace::open(root_dir)?;
            Ok(AppResponse::Model(ws.model))
        }
        AppCommand::ProjectSave { root_dir } => {
            let ws = ProjectWorkspace::open(root_dir)?;
            ws.save()?;
            Ok(AppResponse::Manifest(ws.manifest))
        }
        AppCommand::HarImport {
            root_dir,
            source_name,
            har_bytes,
        } => {
            let mut ws = ProjectWorkspace::open(root_dir)?;
            let src = ws.import_har_bytes(&source_name, &har_bytes)?;
            Ok(AppResponse::Source(src))
        }
        AppCommand::AnalysisStart { root_dir } => {
            let mut ws = ProjectWorkspace::open(root_dir)?;
            ws.analyze()?;
            Ok(AppResponse::Model(ws.model))
        }
        AppCommand::EndpointsList { root_dir } => {
            let ws = ProjectWorkspace::open(root_dir)?;
            Ok(AppResponse::Endpoints(ws.model.endpoints))
        }
        AppCommand::DependenciesList { root_dir } => {
            let ws = ProjectWorkspace::open(root_dir)?;
            Ok(AppResponse::Dependencies(ws.model.dependencies))
        }
        AppCommand::DependenciesConfirm {
            root_dir,
            dependency_id,
        } => {
            let mut ws = ProjectWorkspace::open(root_dir)?;
            ws.set_dependency_status(&dependency_id, ConfirmationStatus::UserConfirmed)?;
            Ok(AppResponse::Dependencies(ws.model.dependencies))
        }
        AppCommand::DependenciesReject {
            root_dir,
            dependency_id,
        } => {
            let mut ws = ProjectWorkspace::open(root_dir)?;
            ws.set_dependency_status(&dependency_id, ConfirmationStatus::Rejected)?;
            Ok(AppResponse::Dependencies(ws.model.dependencies))
        }
        AppCommand::GraphGet { root_dir } => {
            let ws = ProjectWorkspace::open(root_dir)?;
            Ok(AppResponse::Graph(build_endpoint_graph(&ws.model)))
        }
    }
}

fn write_json_atomic<T: Serialize>(target: &Path, value: &T) -> Result<(), HarflowError> {
    let tmp_name = format!(
        ".{}.tmp",
        target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("harflow")
    );
    let tmp_path = target.with_file_name(tmp_name);
    let json = serde_json::to_string_pretty(value)
        .map_err(|err| HarflowError::Storage(format!("serialization failed: {err}")))?;
    fs::write(&tmp_path, json)?;
    fs::rename(&tmp_path, target)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use harflow_model::{DependencyType, ValueLocation, ValueReference};

    use super::*;

    #[test]
    fn creates_imports_analyzes_and_preserves_user_overrides() {
        let dir = std::env::temp_dir().join(format!("harflow_proj_test_{}", Uuid::new_v4()));
        let mut ws = ProjectWorkspace::create(&dir, "Test Project").expect("create workspace");

        let sample_har = br#"{
          "log": {
            "version": "1.2",
            "entries": [
              {
                "request": {
                  "method": "GET",
                  "url": "https://api.example.test/v1/orders/42",
                  "headers": [{ "name": "Authorization", "value": "Bearer hf_synthetic_tok_42" }]
                },
                "response": { "status": 200, "content": { "mimeType": "application/json", "text": "{\"id\":42}" } }
              }
            ]
          }
        }"#;

        let src = ws
            .import_har_bytes("sample.har", sample_har)
            .expect("import");
        assert_eq!(src.entry_count, 1);
        assert_eq!(src.redacted_secrets_count, 1);

        ws.model.dependencies.push(Dependency {
            id: "dep-1".into(),
            source: ValueReference {
                session_id: "sess-0001".into(),
                request_id: "sess-0001-req-0001".into(),
                location: ValueLocation::ResponseBody,
                json_path_or_key: "$.id".into(),
            },
            target: ValueReference {
                session_id: "sess-0001".into(),
                request_id: "sess-0001-req-0001".into(),
                location: ValueLocation::Path,
                json_path_or_key: "id".into(),
            },
            dependency_type: DependencyType::ResponseToPath,
            transformation: None,
            confidence: 0.78,
            status: ConfirmationStatus::Likely,
            evidence: Vec::new(),
            observed_session_ids: vec!["sess-0001".into()],
        });
        ws.save().expect("save");

        ws.set_dependency_status("dep-1", ConfirmationStatus::UserConfirmed)
            .expect("confirm dep");
        ws.analyze().expect("re-analyze");

        let reopened = ProjectWorkspace::open(&dir).expect("reopen");
        assert_eq!(
            reopened.model.dependencies[0].status,
            ConfirmationStatus::UserConfirmed
        );
        assert_eq!(reopened.model.endpoints.len(), 1);
        assert_eq!(reopened.model.endpoints[0].path_template, "/v1/orders/{id}");

        let sessions_disk = fs::read_to_string(dir.join("sessions.json")).expect("read sessions");
        assert!(!sessions_disk.contains("hf_synthetic_tok_42"));

        let _ = fs::remove_dir_all(&dir);
    }
}
