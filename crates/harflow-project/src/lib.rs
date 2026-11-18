use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use chrono::Utc;
use harflow_classifier::{filter_sessions, preview_cleaning, CleanerConfig, CleanerPreview};
use harflow_core::{HarflowError, SCHEMA_VERSION};
use harflow_dependencies::detect_dependencies;
use harflow_endpoints::discover_endpoints_with_overrides;
use harflow_graph::{analyze_graph_cycles, build_graph, GraphKind, GraphViewModel};
use harflow_har::{HarStreamSummary, ImportLimits};
use harflow_model::{ApiModel, ConfirmationStatus, Dependency, Endpoint, SessionSource, Workflow};
use harflow_normalizer::{normalize_har_reader, NormalizedSession};
use harflow_schema::infer_endpoint_schemas;
use harflow_sessions::{
    analyze_session_lifecycle, compare_sessions, detect_observed_auth, SessionComparisonReport,
    SessionLifecycleReport,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod sqlite_store;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSettings {
    pub retain_original_har: bool,
    pub max_har_size_bytes: usize,
    pub offline_only: bool,
    #[serde(default)]
    pub cleaner: CleanerConfig,
    #[serde(default)]
    pub endpoint_overrides: BTreeMap<String, String>,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            retain_original_har: false,
            max_har_size_bytes: ImportLimits::default().default_max_bytes,
            offline_only: true,
            cleaner: CleanerConfig::default(),
            endpoint_overrides: BTreeMap::new(),
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

/// Manages a portable local project directory on disk backed by SQLite indexing and atomic JSON manifests.
#[derive(Debug, Clone)]
pub struct ProjectWorkspace {
    pub root_dir: PathBuf,
    pub manifest: ProjectManifest,
    pub model: ApiModel,
    pub sessions: Vec<NormalizedSession>,
    pub skipped_counts: BTreeMap<String, usize>,
}

impl ProjectWorkspace {
    /// Creates and persists a new empty project directory with SQLite storage and atomic JSON manifests.
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
            skipped_counts: BTreeMap::new(),
        };
        workspace.save()?;
        Ok(workspace)
    }

    /// Opens an existing HARFlow project directory (including Phase 0 projects without `harflow.sqlite`).
    pub fn open(root_dir: impl AsRef<Path>) -> Result<Self, HarflowError> {
        let root = root_dir.as_ref().to_path_buf();
        let manifest_path = root.join("project.json");
        let model_path = root.join("api-model.json");
        let db_path = root.join("harflow.sqlite");
        let legacy_sessions_path = root.join("sessions.json");

        let manifest_raw = fs::read_to_string(&manifest_path)?;
        let mut manifest: ProjectManifest = serde_json::from_str(&manifest_raw)
            .map_err(|err| HarflowError::InvalidSource(format!("invalid project.json: {err}")))?;

        let model_raw = fs::read_to_string(&model_path)?;
        let model: ApiModel = serde_json::from_str(&model_raw)
            .map_err(|err| HarflowError::InvalidSource(format!("invalid api-model.json: {err}")))?;

        let (sessions, skipped_counts, needs_sqlite_backfill) = if db_path.exists() {
            let (loaded_sessions, loaded_skipped) =
                sqlite_store::load_sessions_from_sqlite(&db_path, &manifest.id, &manifest.sources)?;
            let sqlite_overrides =
                sqlite_store::load_overrides_from_sqlite(&db_path, &manifest.id)?;
            for (dep_id, status) in sqlite_overrides {
                manifest.user_overrides.entry(dep_id).or_insert(status);
            }
            (loaded_sessions, loaded_skipped, false)
        } else if legacy_sessions_path.exists() {
            let sessions_raw = fs::read_to_string(&legacy_sessions_path)?;
            let loaded: Vec<NormalizedSession> =
                serde_json::from_str(&sessions_raw).map_err(|err| {
                    HarflowError::InvalidSource(format!("invalid sessions.json: {err}"))
                })?;
            (loaded, BTreeMap::new(), true)
        } else {
            (Vec::new(), BTreeMap::new(), true)
        };

        let ws = Self {
            root_dir: root,
            manifest,
            model,
            sessions,
            skipped_counts,
        };

        if needs_sqlite_backfill {
            sqlite_store::sync_workspace_to_sqlite(
                &db_path,
                &ws.manifest,
                &ws.model,
                &ws.sessions,
                &ws.skipped_counts,
            )?;
        }

        Ok(ws)
    }

    /// Persists `project.json`, `api-model.json`, and the `harflow.sqlite` local index atomically.
    pub fn save(&self) -> Result<(), HarflowError> {
        fs::create_dir_all(&self.root_dir)?;
        write_json_atomic(&self.root_dir.join("project.json"), &self.manifest)?;
        write_json_atomic(&self.root_dir.join("api-model.json"), &self.model)?;
        sqlite_store::sync_workspace_to_sqlite(
            &self.root_dir.join("harflow.sqlite"),
            &self.manifest,
            &self.model,
            &self.sessions,
            &self.skipped_counts,
        )?;
        Ok(())
    }

    /// Streams, normalizes, and redacts a HAR 1.2 or `.har.gz` reader without retaining raw secrets.
    pub fn import_har_reader<R: Read>(
        &mut self,
        source_name: &str,
        reader: R,
        custom_limits: Option<ImportLimits>,
    ) -> Result<(SessionSource, HarStreamSummary), HarflowError> {
        let limits = custom_limits.unwrap_or_else(|| ImportLimits {
            default_max_bytes: self.manifest.settings.max_har_size_bytes,
            ..ImportLimits::default()
        });
        let session_id = format!("sess-{:04}", self.sessions.len() + 1);
        let (normalized, summary) =
            normalize_har_reader(&session_id, source_name, reader, &limits)?;

        let source = SessionSource {
            id: session_id.clone(),
            name: source_name.to_string(),
            har_version: Some(normalized.har_version.clone()),
            imported_at: Utc::now().to_rfc3339(),
            entry_count: normalized.entries.len(),
            redacted_secrets_count: normalized.redacted_secrets_count,
        };

        self.skipped_counts
            .insert(session_id, summary.skipped_entries.len());
        self.sessions.push(normalized);
        self.manifest.sources.push(source.clone());
        self.model.sources.push(source.clone());
        self.refresh_statistics();
        self.manifest.updated_at = Utc::now().to_rfc3339();
        self.save()?;

        Ok((source, summary))
    }

    /// Imports and sanitizes in-memory HAR 1.2 bytes without retaining raw credentials.
    pub fn import_har_bytes(
        &mut self,
        source_name: &str,
        bytes: &[u8],
    ) -> Result<SessionSource, HarflowError> {
        let (source, _summary) = self.import_har_reader(source_name, bytes, None)?;
        Ok(source)
    }

    /// Runs deterministic traffic cleaning, endpoint discovery, schema inference, dependency detection,
    /// workflow synthesis, and auth/cycle detection while preserving user overrides.
    pub fn analyze(&mut self) -> Result<(), HarflowError> {
        let preserved_overridden: BTreeMap<String, Dependency> = self
            .model
            .dependencies
            .iter()
            .filter(|d| self.manifest.user_overrides.contains_key(&d.id))
            .map(|d| (d.id.clone(), d.clone()))
            .collect();

        let (filtered_sessions, preview) =
            filter_sessions(&self.sessions, &self.manifest.settings.cleaner);

        self.model.metadata.excluded_requests = preview.excluded_entries;
        self.model.endpoints = discover_endpoints_with_overrides(
            &filtered_sessions,
            &self.manifest.settings.endpoint_overrides,
        );
        self.model.schemas = infer_endpoint_schemas(&mut self.model.endpoints, &filtered_sessions);
        self.model.auth_flows = detect_observed_auth(&filtered_sessions);

        let mut detected = detect_dependencies(&filtered_sessions, &self.model.endpoints);
        let mut seen_ids: BTreeSet<String> = detected.iter().map(|d| d.id.clone()).collect();
        for dep in &mut detected {
            if let Some(status) = self.manifest.user_overrides.get(&dep.id) {
                dep.status = *status;
            }
        }
        for (id, mut dep) in preserved_overridden {
            if !seen_ids.contains(&id) {
                if let Some(status) = self.manifest.user_overrides.get(&id) {
                    dep.status = *status;
                }
                seen_ids.insert(id);
                detected.push(dep);
            }
        }
        self.model.dependencies = detected;
        self.model.workflows =
            synthesize_workflows(&self.model.endpoints, &self.model.dependencies);

        let cycle_analysis = analyze_graph_cycles(&self.model);
        self.model
            .diagnostics
            .retain(|d| d.code != "DEPENDENCY_CYCLE_CONTRADICTION");
        self.model.diagnostics.extend(cycle_analysis.diagnostics);

        self.model.metadata.generated_at = Utc::now().to_rfc3339();
        self.refresh_statistics();
        self.manifest.updated_at = Utc::now().to_rfc3339();
        self.save()?;
        sqlite_store::record_analysis_run(&self.root_dir.join("harflow.sqlite"), &self.manifest)?;
        Ok(())
    }

    /// Computes a non-destructive preview of how the configured Smart Traffic Cleaner filters entries.
    #[must_use]
    pub fn preview_cleaning(&self) -> CleanerPreview {
        preview_cleaning(&self.sessions, &self.manifest.settings.cleaner)
    }

    /// Computes the Session Lifecycle & Auth Intelligence report on kept entries.
    #[must_use]
    pub fn session_lifecycle(&self) -> SessionLifecycleReport {
        let (filtered_sessions, _) =
            filter_sessions(&self.sessions, &self.manifest.settings.cleaner);
        analyze_session_lifecycle(&filtered_sessions, &self.model.dependencies)
    }

    /// Computes the Multi-HAR comparison report across normalized project sessions.
    #[must_use]
    pub fn compare_sessions(&self) -> SessionComparisonReport {
        let (filtered_sessions, _) =
            filter_sessions(&self.sessions, &self.manifest.settings.cleaner);
        compare_sessions(&filtered_sessions, &self.model.dependencies)
    }

    /// Builds a requested `GraphViewModel` view from the current project model & sessions.
    #[must_use]
    pub fn build_graph(&self, kind: GraphKind) -> GraphViewModel {
        let (filtered_sessions, _) =
            filter_sessions(&self.sessions, &self.manifest.settings.cleaner);
        build_graph(&self.model, &filtered_sessions, kind)
    }

    /// Logically excludes a domain from analysis without deleting underlying captured entries.
    pub fn exclude_domain(&mut self, domain: &str) -> Result<CleanerPreview, HarflowError> {
        let norm = domain.trim().to_ascii_lowercase();
        self.manifest
            .settings
            .cleaner
            .included_domains
            .remove(&norm);
        self.manifest.settings.cleaner.excluded_domains.insert(norm);
        self.manifest.updated_at = Utc::now().to_rfc3339();
        self.save()?;
        Ok(self.preview_cleaning())
    }

    /// Explicitly includes a domain in analysis (except static assets & CORS preflight).
    pub fn include_domain(&mut self, domain: &str) -> Result<CleanerPreview, HarflowError> {
        let norm = domain.trim().to_ascii_lowercase();
        self.manifest
            .settings
            .cleaner
            .excluded_domains
            .remove(&norm);
        self.manifest.settings.cleaner.included_domains.insert(norm);
        self.manifest.updated_at = Utc::now().to_rfc3339();
        self.save()?;
        Ok(self.preview_cleaning())
    }

    /// Logically excludes a single request by ID without removing it from SQLite.
    pub fn exclude_request(&mut self, request_id: &str) -> Result<CleanerPreview, HarflowError> {
        let id = request_id.trim().to_string();
        self.manifest.settings.cleaner.keep_request_ids.remove(&id);
        self.manifest
            .settings
            .cleaner
            .exclude_request_ids
            .insert(id);
        self.manifest.updated_at = Utc::now().to_rfc3339();
        self.save()?;
        Ok(self.preview_cleaning())
    }

    /// Restores a previously excluded or noise-filtered request into active analysis.
    pub fn restore_request(&mut self, request_id: &str) -> Result<CleanerPreview, HarflowError> {
        let id = request_id.trim().to_string();
        self.manifest
            .settings
            .cleaner
            .exclude_request_ids
            .remove(&id);
        self.manifest.settings.cleaner.keep_request_ids.insert(id);
        self.manifest.updated_at = Utc::now().to_rfc3339();
        self.save()?;
        Ok(self.preview_cleaning())
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
    GraphGet {
        root_dir: PathBuf,
        #[serde(default)]
        kind: Option<GraphKind>,
    },
    #[serde(rename = "sessions.lifecycle")]
    SessionsLifecycle { root_dir: PathBuf },
    #[serde(rename = "sessions.compare")]
    SessionsCompare { root_dir: PathBuf },
    #[serde(rename = "cleaner.preview")]
    CleanerPreview { root_dir: PathBuf },
    #[serde(rename = "cleaner.update")]
    CleanerUpdate {
        root_dir: PathBuf,
        #[serde(default)]
        exclude_domain: Option<String>,
        #[serde(default)]
        include_domain: Option<String>,
        #[serde(default)]
        exclude_request: Option<String>,
        #[serde(default)]
        restore_request: Option<String>,
    },
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
    Cleaner(CleanerPreview),
    SessionLifecycle(SessionLifecycleReport),
    SessionComparison(SessionComparisonReport),
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
        AppCommand::GraphGet { root_dir, kind } => {
            let ws = ProjectWorkspace::open(root_dir)?;
            Ok(AppResponse::Graph(
                ws.build_graph(kind.unwrap_or(GraphKind::Endpoint)),
            ))
        }
        AppCommand::SessionsLifecycle { root_dir } => {
            let ws = ProjectWorkspace::open(root_dir)?;
            Ok(AppResponse::SessionLifecycle(ws.session_lifecycle()))
        }
        AppCommand::SessionsCompare { root_dir } => {
            let ws = ProjectWorkspace::open(root_dir)?;
            Ok(AppResponse::SessionComparison(ws.compare_sessions()))
        }
        AppCommand::CleanerPreview { root_dir } => {
            let ws = ProjectWorkspace::open(root_dir)?;
            Ok(AppResponse::Cleaner(ws.preview_cleaning()))
        }
        AppCommand::CleanerUpdate {
            root_dir,
            exclude_domain,
            include_domain,
            exclude_request,
            restore_request,
        } => {
            let mut ws = ProjectWorkspace::open(root_dir)?;
            if let Some(d) = exclude_domain {
                ws.exclude_domain(&d)?;
            }
            if let Some(d) = include_domain {
                ws.include_domain(&d)?;
            }
            if let Some(r) = exclude_request {
                ws.exclude_request(&r)?;
            }
            if let Some(r) = restore_request {
                ws.restore_request(&r)?;
            }
            Ok(AppResponse::Cleaner(ws.preview_cleaning()))
        }
    }
}

fn synthesize_workflows(endpoints: &[Endpoint], dependencies: &[Dependency]) -> Vec<Workflow> {
    if endpoints.is_empty() || dependencies.is_empty() {
        return Vec::new();
    }

    let mut req_to_ep: BTreeMap<&str, usize> = BTreeMap::new();
    for (idx, ep) in endpoints.iter().enumerate() {
        for req_id in &ep.observed_request_ids {
            req_to_ep.insert(req_id.as_str(), idx);
        }
    }

    let n = endpoints.len();
    let mut adj: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); n];
    let mut undirected: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); n];

    for dep in dependencies {
        if !matches!(
            dep.status,
            ConfirmationStatus::Confirmed
                | ConfirmationStatus::UserConfirmed
                | ConfirmationStatus::Likely
        ) {
            continue;
        }
        let (Some(&u), Some(&v)) = (
            req_to_ep.get(dep.source.request_id.as_str()),
            req_to_ep.get(dep.target.request_id.as_str()),
        ) else {
            continue;
        };
        if u == v {
            continue;
        }
        adj[u].insert(v);
        undirected[u].insert(v);
        undirected[v].insert(u);
    }

    let mut visited = vec![false; n];
    let mut workflows = Vec::new();
    for start in 0..n {
        if visited[start] || undirected[start].is_empty() {
            continue;
        }
        let mut comp = BTreeSet::new();
        let mut q = VecDeque::from([start]);
        visited[start] = true;
        while let Some(u) = q.pop_front() {
            comp.insert(u);
            for &v in &undirected[u] {
                if !visited[v] {
                    visited[v] = true;
                    q.push_back(v);
                }
            }
        }
        if comp.len() < 2 {
            continue;
        }

        // Deterministically sort nodes in component using Kahn topological order bounded by discovery order.
        let mut local_in: BTreeMap<usize, usize> = comp
            .iter()
            .map(|&idx| {
                let deg = adj
                    .iter()
                    .enumerate()
                    .filter(|(src, edges)| comp.contains(src) && edges.contains(&idx))
                    .count();
                (idx, deg)
            })
            .collect();
        let mut topo_q: BTreeSet<usize> = local_in
            .iter()
            .filter_map(|(&idx, &deg)| (deg == 0).then_some(idx))
            .collect();

        let mut ordered = Vec::with_capacity(comp.len());
        let mut remaining = comp;
        while !remaining.is_empty() {
            let next = if let Some(&zero) = topo_q.iter().next() {
                topo_q.remove(&zero);
                zero
            } else {
                *remaining.iter().next().expect("non-empty component")
            };
            remaining.remove(&next);
            ordered.push(endpoints[next].id.clone());
            for &nxt in &adj[next] {
                if remaining.contains(&nxt) {
                    if let Some(d) = local_in.get_mut(&nxt) {
                        *d = d.saturating_sub(1);
                        if *d == 0 {
                            topo_q.insert(nxt);
                        }
                    }
                }
            }
        }

        let first = &endpoints[ordered
            .first()
            .and_then(|id| endpoints.iter().position(|e| &e.id == id))
            .unwrap_or(start)];
        let wf_idx = workflows.len() + 1;
        workflows.push(Workflow {
            id: format!("wf-{wf_idx:02}"),
            name: format!(
                "{} ({} {}) Flow",
                first.host, first.method, first.path_template
            ),
            step_endpoint_ids: ordered,
        });
    }

    workflows
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
    use rusqlite::Connection;

    use super::*;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates dir")
            .parent()
            .expect("repo root")
            .to_path_buf()
    }

    #[test]
    fn creates_imports_analyzes_persists_sqlite_and_preserves_overrides() {
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
        assert_eq!(
            reopened.model.endpoints[0].path_template,
            "/v1/orders/{order_id}"
        );

        // Verify all 11 SRS tables and recorded analysis run exist in harflow.sqlite.
        let db_path = dir.join("harflow.sqlite");
        let conn = Connection::open(&db_path).expect("open sqlite");
        for table in [
            "projects",
            "sources",
            "requests",
            "endpoints",
            "schemas",
            "dependencies",
            "evidence",
            "user_overrides",
            "analysis_runs",
            "validation_runs",
            "generated_artifacts",
        ] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |row| row.get(0),
                )
                .expect("query sqlite_master");
            assert_eq!(count, 1, "missing required table {table}");
        }
        let runs: i64 = conn
            .query_row("SELECT COUNT(*) FROM analysis_runs", [], |r| r.get(0))
            .expect("query analysis_runs");
        assert_eq!(runs, 1);
        drop(conn);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sqlite_storage_never_contains_raw_secrets_and_creates_migration_backup() {
        let dir = std::env::temp_dir().join(format!("harflow_sec_db_{}", Uuid::new_v4()));
        let mut ws = ProjectWorkspace::create(&dir, "Secret Hygiene").expect("create workspace");
        let fixture = repo_root().join("fixtures/synthetic-secrets/redaction-sample.har.json");
        let raw_bytes = fs::read(&fixture).expect("read fixture");

        ws.import_har_bytes("redaction-sample.har.json", &raw_bytes)
            .expect("import redaction fixture");
        ws.analyze().expect("analyze");

        let db_path = dir.join("harflow.sqlite");
        {
            let conn = Connection::open(&db_path).expect("open sqlite");
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
                .expect("checkpoint wal");
        }

        let db_bytes = fs::read(&db_path).expect("read sqlite bytes");
        let db_lossy = String::from_utf8_lossy(&db_bytes);
        for raw_secret in [
            "hf_synthetic_bearer_secret_0001",
            "hf_synthetic_cookie_secret_0002",
            "hf_synthetic_password_0003",
            "hf_synthetic_api_key_0004",
        ] {
            assert!(
                !db_lossy.contains(raw_secret),
                "raw secret '{raw_secret}' leaked into harflow.sqlite"
            );
        }

        // Trigger migration from v1 to v2 and verify pre-migration backup exists.
        {
            let migrated = sqlite_store::open_and_migrate_db(&db_path, 2).expect("migrate v1->v2");
            let ver: i32 = migrated
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .expect("read user_version");
            assert_eq!(ver, 2);
        }
        assert!(dir.join("harflow.sqlite.bak.v1").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn migrates_legacy_phase0_sessions_json_project_seamlessly() {
        let dir = std::env::temp_dir().join(format!("harflow_legacy_{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("mkdir");

        let mut manifest = ProjectWorkspace::create(&dir, "Legacy")
            .expect("create")
            .manifest;
        fs::remove_file(dir.join("harflow.sqlite")).expect("remove sqlite to simulate phase 0");

        let session = NormalizedSession {
            session_id: "sess-0001".into(),
            source_name: "legacy.har".into(),
            har_version: "1.2".into(),
            entries: vec![harflow_normalizer::NormalizedEntry {
                id: "sess-0001-req-0001".into(),
                session_id: "sess-0001".into(),
                method: "GET".into(),
                url: "https://api.example.test/v1/health".into(),
                host: "api.example.test".into(),
                path: "/v1/health".into(),
                response_status: 200,
                ..harflow_normalizer::NormalizedEntry::default()
            }],
            redacted_secrets_count: 0,
        };
        manifest.sources.push(SessionSource {
            id: "sess-0001".into(),
            name: "legacy.har".into(),
            har_version: Some("1.2".into()),
            imported_at: Utc::now().to_rfc3339(),
            entry_count: 1,
            redacted_secrets_count: 0,
        });
        write_json_atomic(&dir.join("project.json"), &manifest).expect("write manifest");
        write_json_atomic(&dir.join("sessions.json"), &vec![session]).expect("write legacy json");

        let mut reopened = ProjectWorkspace::open(&dir).expect("open legacy project");
        assert!(dir.join("harflow.sqlite").exists());
        assert_eq!(reopened.sessions.len(), 1);
        reopened.analyze().expect("analyze legacy project");
        assert_eq!(reopened.model.endpoints.len(), 1);

        let _ = fs::remove_dir_all(&dir);
    }
}
