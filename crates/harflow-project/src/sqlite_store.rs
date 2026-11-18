use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use chrono::Utc;
use harflow_core::HarflowError;
use harflow_model::{ApiModel, ConfirmationStatus, SessionSource};
use harflow_normalizer::{NormalizedEntry, NormalizedSession};
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::ProjectManifest;

/// Current SQLite project index schema version.
pub const DB_SCHEMA_VERSION: i32 = 1;

const SCHEMA_V1_SQL: &str = r"
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    schema_version TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    settings_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sources (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    har_version TEXT,
    imported_at TEXT NOT NULL,
    entry_count INTEGER NOT NULL,
    redacted_secrets_count INTEGER NOT NULL,
    skipped_entries_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS requests (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    method TEXT NOT NULL,
    url TEXT NOT NULL,
    host TEXT NOT NULL,
    path TEXT NOT NULL,
    status INTEGER NOT NULL,
    mime_type TEXT NOT NULL,
    duration_ms REAL NOT NULL,
    transfer_size INTEGER NOT NULL,
    started_date_time TEXT NOT NULL,
    entry_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_requests_session ON requests(project_id, session_id);

CREATE TABLE IF NOT EXISTS endpoints (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    method TEXT NOT NULL,
    host TEXT NOT NULL,
    path_template TEXT NOT NULL,
    category TEXT NOT NULL,
    confidence REAL NOT NULL,
    endpoint_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS schemas (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    root_type TEXT NOT NULL,
    observation_count INTEGER NOT NULL,
    schema_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS dependencies (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    dependency_type TEXT NOT NULL,
    confidence REAL NOT NULL,
    status TEXT NOT NULL,
    dependency_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS evidence (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    dependency_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    request_id TEXT NOT NULL,
    rule_id TEXT NOT NULL,
    description TEXT NOT NULL,
    observed_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS user_overrides (
    dependency_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    status TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS analysis_runs (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    ran_at TEXT NOT NULL,
    total_requests INTEGER NOT NULL,
    unique_endpoints INTEGER NOT NULL,
    dependencies_count INTEGER NOT NULL,
    status TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS validation_runs (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    ran_at TEXT NOT NULL,
    mode TEXT NOT NULL,
    total_checks INTEGER NOT NULL,
    passed INTEGER NOT NULL,
    failed INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS generated_artifacts (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    format TEXT NOT NULL,
    output_path TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);
";

fn db_err(err: rusqlite::Error) -> HarflowError {
    HarflowError::Storage(format!("sqlite error: {err}"))
}

/// Opens the local project SQLite index and applies forward migrations with a pre-migration backup.
pub fn open_and_migrate_db(
    db_path: &Path,
    target_version: i32,
) -> Result<Connection, HarflowError> {
    if db_path.exists() {
        let probe = Connection::open(db_path).map_err(db_err)?;
        let current_version: i32 = probe
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(db_err)?;
        drop(probe);

        if current_version > target_version {
            return Err(HarflowError::InvalidSource(format!(
                "project database schema version {current_version} is newer than supported {target_version}"
            )));
        }

        if current_version > 0 && current_version < target_version {
            let backup_name = format!(
                "{}.bak.v{current_version}",
                db_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("harflow.sqlite")
            );
            let backup_path = db_path.with_file_name(backup_name);
            fs::copy(db_path, &backup_path)?;
        }
    }

    let mut conn = Connection::open(db_path).map_err(db_err)?;
    conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")
        .map_err(db_err)?;

    let current_version: i32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(db_err)?;

    if current_version < target_version {
        let tx = conn.transaction().map_err(db_err)?;
        for ver in (current_version + 1)..=target_version {
            match ver {
                1 => {
                    tx.execute_batch(SCHEMA_V1_SQL).map_err(db_err)?;
                }
                2 => {
                    // Forward metadata-only schema bump hook; preserves all v1 tables.
                }
                other => {
                    return Err(HarflowError::InvalidSource(format!(
                        "unsupported sqlite migration target version {other}"
                    )));
                }
            }
            tx.execute(
                "INSERT OR REPLACE INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![ver, Utc::now().to_rfc3339()],
            )
            .map_err(db_err)?;
            tx.pragma_update(None, "user_version", ver)
                .map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
    }

    Ok(conn)
}

/// Synchronizes project metadata, sanitized sessions/requests, discovered entities, and overrides into SQLite.
pub fn sync_workspace_to_sqlite(
    db_path: &Path,
    manifest: &ProjectManifest,
    model: &ApiModel,
    sessions: &[NormalizedSession],
    skipped_counts: &BTreeMap<String, usize>,
) -> Result<(), HarflowError> {
    let mut conn = open_and_migrate_db(db_path, DB_SCHEMA_VERSION)?;
    let tx = conn.transaction().map_err(db_err)?;

    let settings_json = serde_json::to_string(&manifest.settings)
        .map_err(|e| HarflowError::Storage(format!("serialize settings: {e}")))?;

    tx.execute(
        "INSERT OR REPLACE INTO projects (id, name, schema_version, created_at, updated_at, settings_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            manifest.id,
            manifest.name,
            manifest.schema_version,
            manifest.created_at,
            manifest.updated_at,
            settings_json
        ],
    )
    .map_err(db_err)?;

    tx.execute(
        "DELETE FROM sources WHERE project_id = ?1",
        params![manifest.id],
    )
    .map_err(db_err)?;
    tx.execute(
        "DELETE FROM requests WHERE project_id = ?1",
        params![manifest.id],
    )
    .map_err(db_err)?;
    tx.execute(
        "DELETE FROM endpoints WHERE project_id = ?1",
        params![manifest.id],
    )
    .map_err(db_err)?;
    tx.execute(
        "DELETE FROM schemas WHERE project_id = ?1",
        params![manifest.id],
    )
    .map_err(db_err)?;
    tx.execute(
        "DELETE FROM evidence WHERE dependency_id IN (SELECT id FROM dependencies WHERE project_id = ?1)",
        params![manifest.id],
    )
    .map_err(db_err)?;
    tx.execute(
        "DELETE FROM dependencies WHERE project_id = ?1",
        params![manifest.id],
    )
    .map_err(db_err)?;
    tx.execute(
        "DELETE FROM user_overrides WHERE project_id = ?1",
        params![manifest.id],
    )
    .map_err(db_err)?;

    {
        let mut stmt = tx
            .prepare_cached(
                "INSERT INTO sources (id, project_id, name, har_version, imported_at, entry_count, redacted_secrets_count, skipped_entries_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )
            .map_err(db_err)?;
        for src in &manifest.sources {
            let skipped = skipped_counts.get(&src.id).copied().unwrap_or(0);
            stmt.execute(params![
                src.id,
                manifest.id,
                src.name,
                src.har_version,
                src.imported_at,
                src.entry_count as i64,
                src.redacted_secrets_count as i64,
                skipped as i64,
            ])
            .map_err(db_err)?;
        }
    }

    {
        let mut stmt = tx
            .prepare_cached(
                "INSERT INTO requests (id, project_id, session_id, method, url, host, path, status, mime_type, duration_ms, transfer_size, started_date_time, entry_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            )
            .map_err(db_err)?;
        for session in sessions {
            for entry in &session.entries {
                let entry_json = serde_json::to_string(entry)
                    .map_err(|e| HarflowError::Storage(format!("serialize entry: {e}")))?;
                stmt.execute(params![
                    entry.id,
                    manifest.id,
                    session.session_id,
                    entry.method,
                    entry.url,
                    entry.host,
                    entry.path,
                    i64::from(entry.response_status),
                    entry.response_mime_type,
                    entry.duration_ms,
                    entry.transfer_size as i64,
                    entry.started_date_time,
                    entry_json,
                ])
                .map_err(db_err)?;
            }
        }
    }

    {
        let mut stmt = tx
            .prepare_cached(
                "INSERT INTO endpoints (id, project_id, method, host, path_template, category, confidence, endpoint_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )
            .map_err(db_err)?;
        for ep in &model.endpoints {
            let ep_json = serde_json::to_string(ep)
                .map_err(|e| HarflowError::Storage(format!("serialize endpoint: {e}")))?;
            stmt.execute(params![
                ep.id,
                manifest.id,
                ep.method,
                ep.host,
                ep.path_template,
                ep.category,
                ep.confidence,
                ep_json,
            ])
            .map_err(db_err)?;
        }
    }

    {
        let mut stmt = tx
            .prepare_cached(
                "INSERT INTO schemas (id, project_id, name, root_type, observation_count, schema_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .map_err(db_err)?;
        for schema in &model.schemas {
            let schema_json = serde_json::to_string(schema)
                .map_err(|e| HarflowError::Storage(format!("serialize schema: {e}")))?;
            stmt.execute(params![
                schema.id,
                manifest.id,
                schema.name,
                schema.root_type,
                schema.observation_count as i64,
                schema_json,
            ])
            .map_err(db_err)?;
        }
    }

    {
        let mut dep_stmt = tx
            .prepare_cached(
                "INSERT INTO dependencies (id, project_id, dependency_type, confidence, status, dependency_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .map_err(db_err)?;
        let mut ev_stmt = tx
            .prepare_cached(
                "INSERT INTO evidence (dependency_id, session_id, request_id, rule_id, description, observed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .map_err(db_err)?;
        for dep in &model.dependencies {
            let dep_json = serde_json::to_string(dep)
                .map_err(|e| HarflowError::Storage(format!("serialize dependency: {e}")))?;
            let dep_type = serde_json::to_string(&dep.dependency_type)
                .unwrap_or_else(|_| "\"unknown\"".to_string())
                .trim_matches('"')
                .to_string();
            let status_str = serde_json::to_string(&dep.status)
                .unwrap_or_else(|_| "\"unknown\"".to_string())
                .trim_matches('"')
                .to_string();
            dep_stmt
                .execute(params![
                    dep.id,
                    manifest.id,
                    dep_type,
                    dep.confidence,
                    status_str,
                    dep_json,
                ])
                .map_err(db_err)?;

            for ev in &dep.evidence {
                ev_stmt
                    .execute(params![
                        dep.id,
                        ev.session_id,
                        ev.request_id,
                        ev.rule_id,
                        ev.description,
                        ev.observed_at,
                    ])
                    .map_err(db_err)?;
            }
        }
    }

    {
        let mut stmt = tx
            .prepare_cached(
                "INSERT INTO user_overrides (dependency_id, project_id, status, updated_at)
                 VALUES (?1, ?2, ?3, ?4)",
            )
            .map_err(db_err)?;
        for (dep_id, status) in &manifest.user_overrides {
            let status_str = serde_json::to_string(status)
                .unwrap_or_else(|_| "\"unknown\"".to_string())
                .trim_matches('"')
                .to_string();
            stmt.execute(params![
                dep_id,
                manifest.id,
                status_str,
                manifest.updated_at
            ])
            .map_err(db_err)?;
        }
    }

    tx.commit().map_err(db_err)?;
    Ok(())
}

/// Records an executed analysis run in `analysis_runs`.
pub fn record_analysis_run(db_path: &Path, manifest: &ProjectManifest) -> Result<(), HarflowError> {
    let conn = open_and_migrate_db(db_path, DB_SCHEMA_VERSION)?;
    conn.execute(
        "INSERT INTO analysis_runs (id, project_id, ran_at, total_requests, unique_endpoints, dependencies_count, status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            Uuid::new_v4().to_string(),
            manifest.id,
            Utc::now().to_rfc3339(),
            manifest.statistics.total_requests as i64,
            manifest.statistics.unique_endpoints as i64,
            manifest.statistics.dependencies as i64,
            "completed",
        ],
    )
    .map_err(db_err)?;
    Ok(())
}

/// Loads sanitized sessions and their entries from SQLite.
pub fn load_sessions_from_sqlite(
    db_path: &Path,
    project_id: &str,
    sources: &[SessionSource],
) -> Result<(Vec<NormalizedSession>, BTreeMap<String, usize>), HarflowError> {
    let conn = open_and_migrate_db(db_path, DB_SCHEMA_VERSION)?;

    let mut skipped_counts = BTreeMap::new();
    let mut source_stmt = conn
        .prepare("SELECT id, skipped_entries_count FROM sources WHERE project_id = ?1 ORDER BY rowid ASC")
        .map_err(db_err)?;
    let source_rows = source_stmt
        .query_map(params![project_id], |row| {
            let id: String = row.get(0)?;
            let skipped: i64 = row.get(1)?;
            Ok((id, usize::try_from(skipped).unwrap_or(0)))
        })
        .map_err(db_err)?;
    for item in source_rows {
        let (id, skipped) = item.map_err(db_err)?;
        skipped_counts.insert(id, skipped);
    }

    let mut entries_by_session: BTreeMap<String, Vec<NormalizedEntry>> = BTreeMap::new();
    let mut req_stmt = conn
        .prepare(
            "SELECT session_id, entry_json FROM requests WHERE project_id = ?1 ORDER BY rowid ASC",
        )
        .map_err(db_err)?;
    let req_rows = req_stmt
        .query_map(params![project_id], |row| {
            let sid: String = row.get(0)?;
            let json_str: String = row.get(1)?;
            Ok((sid, json_str))
        })
        .map_err(db_err)?;

    for item in req_rows {
        let (sid, json_str) = item.map_err(db_err)?;
        let entry: NormalizedEntry = serde_json::from_str(&json_str).map_err(|err| {
            HarflowError::InvalidSource(format!("corrupt request entry_json in sqlite: {err}"))
        })?;
        entries_by_session.entry(sid).or_default().push(entry);
    }

    let sessions = sources
        .iter()
        .map(|src| NormalizedSession {
            session_id: src.id.clone(),
            source_name: src.name.clone(),
            har_version: src.har_version.clone().unwrap_or_else(|| "1.2".to_string()),
            entries: entries_by_session.remove(&src.id).unwrap_or_default(),
            redacted_secrets_count: src.redacted_secrets_count,
        })
        .collect();

    Ok((sessions, skipped_counts))
}

/// Loads persisted user dependency overrides from SQLite.
pub fn load_overrides_from_sqlite(
    db_path: &Path,
    project_id: &str,
) -> Result<BTreeMap<String, ConfirmationStatus>, HarflowError> {
    let conn = open_and_migrate_db(db_path, DB_SCHEMA_VERSION)?;
    let mut stmt = conn
        .prepare("SELECT dependency_id, status FROM user_overrides WHERE project_id = ?1")
        .map_err(db_err)?;
    let rows = stmt
        .query_map(params![project_id], |row| {
            let dep_id: String = row.get(0)?;
            let status_str: String = row.get(1)?;
            Ok((dep_id, status_str))
        })
        .map_err(db_err)?;

    let mut map = BTreeMap::new();
    for item in rows {
        let (dep_id, status_str) = item.map_err(db_err)?;
        if let Ok(parsed) = serde_json::from_str::<ConfirmationStatus>(&format!("\"{status_str}\""))
        {
            map.insert(dep_id, parsed);
        }
    }
    Ok(map)
}
