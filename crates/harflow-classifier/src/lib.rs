use std::collections::{BTreeMap, BTreeSet};

use harflow_normalizer::{NormalizedEntry, NormalizedSession};
use serde::{Deserialize, Serialize};

/// All 13 traffic classification categories specified in SRS M04.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrafficCategory {
    Api,
    Authentication,
    Analytics,
    Advertising,
    Telemetry,
    StaticAssets,
    Images,
    Fonts,
    Css,
    JavaScript,
    Media,
    Documents,
    Other,
}

impl TrafficCategory {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::Authentication => "authentication",
            Self::Analytics => "analytics",
            Self::Advertising => "advertising",
            Self::Telemetry => "telemetry",
            Self::StaticAssets => "static_assets",
            Self::Images => "images",
            Self::Fonts => "fonts",
            Self::Css => "css",
            Self::JavaScript => "javascript",
            Self::Media => "media",
            Self::Documents => "documents",
            Self::Other => "other",
        }
    }
}

/// Deterministic classification result paired with an explainable reason for the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassificationResult {
    pub category: TrafficCategory,
    pub reason: String,
}

/// Explicit cleaner action for custom rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterAction {
    Keep,
    Exclude,
}

/// User-defined rule evaluated before default domain and category filters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomCleanerRule {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_suffix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    pub action: FilterAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_category: Option<TrafficCategory>,
}

/// Persistent Smart Traffic Cleaner configuration allowing logical filtering and restoration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanerConfig {
    pub keep_categories: BTreeSet<TrafficCategory>,
    #[serde(default)]
    pub included_domains: BTreeSet<String>,
    #[serde(default)]
    pub excluded_domains: BTreeSet<String>,
    #[serde(default)]
    pub keep_request_ids: BTreeSet<String>,
    #[serde(default)]
    pub exclude_request_ids: BTreeSet<String>,
    #[serde(default)]
    pub custom_rules: Vec<CustomCleanerRule>,
}

impl Default for CleanerConfig {
    fn default() -> Self {
        Self {
            keep_categories: BTreeSet::from([
                TrafficCategory::Api,
                TrafficCategory::Authentication,
            ]),
            included_domains: BTreeSet::new(),
            excluded_domains: BTreeSet::new(),
            keep_request_ids: BTreeSet::new(),
            exclude_request_ids: BTreeSet::new(),
            custom_rules: Vec::new(),
        }
    }
}

/// Per-entry explainable cleaner decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryFilterDecision {
    pub entry_id: String,
    pub session_id: String,
    pub method: String,
    pub host: String,
    pub path: String,
    pub category: TrafficCategory,
    pub classification_reason: String,
    pub kept: bool,
    pub filter_reason: String,
}

/// Aggregate preview emitted before or alongside logical traffic cleaning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CleanerPreview {
    pub total_entries: usize,
    pub kept_entries: usize,
    pub excluded_entries: usize,
    pub category_counts: BTreeMap<String, usize>,
    pub decisions: Vec<EntryFilterDecision>,
}

/// Classifies a normalized entry using deterministic MIME, path, method, header, and host signals.
#[must_use]
pub fn classify_entry(entry: &NormalizedEntry) -> ClassificationResult {
    let path_lower = entry.path.to_ascii_lowercase();
    let mime_lower = entry.response_mime_type.to_ascii_lowercase();
    let host_lower = entry.host.to_ascii_lowercase();
    let method_upper = entry.method.trim().to_ascii_uppercase();

    if method_upper == "OPTIONS"
        && entry
            .request_headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("access-control-request-method"))
    {
        return ClassificationResult {
            category: TrafficCategory::Other,
            reason: "CORS preflight request (OPTIONS)".to_string(),
        };
    }

    if path_contains_any(
        &path_lower,
        &[
            "/auth", "/login", "/logout", "/signin", "/signup", "/oauth", "/sso", "/saml",
        ],
    ) || path_lower.ends_with("/token")
        || path_lower.ends_with("/refresh")
    {
        return ClassificationResult {
            category: TrafficCategory::Authentication,
            reason: "Path matches authentication or token lifecycle keywords".to_string(),
        };
    }

    if host_contains_any(
        &host_lower,
        &["doubleclick", "googlesyndication", "adservice", "adsystem"],
    ) || path_contains_any(&path_lower, &["/ads/", "/adserver", "/pixel"])
    {
        return ClassificationResult {
            category: TrafficCategory::Advertising,
            reason: "Host or path matches advertising/tracking signatures".to_string(),
        };
    }

    if host_contains_any(
        &host_lower,
        &[
            "google-analytics",
            "googletagmanager",
            "segment.io",
            "mixpanel",
            "amplitude",
            "posthog",
            "plausible",
            "analytics",
        ],
    ) || path_contains_any(&path_lower, &["/analytics", "/collect"])
    {
        return ClassificationResult {
            category: TrafficCategory::Analytics,
            reason: "Host or path matches analytics endpoint".to_string(),
        };
    }

    if host_contains_any(
        &host_lower,
        &["sentry", "datadog", "newrelic", "bugsnag", "otel"],
    ) || path_contains_any(&path_lower, &["/telemetry", "/metrics", "/traces"])
    {
        return ClassificationResult {
            category: TrafficCategory::Telemetry,
            reason: "Host or path matches telemetry or observability endpoint".to_string(),
        };
    }

    if mime_lower.starts_with("image/")
        || has_any_ext(
            &path_lower,
            &[".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".ico"],
        )
    {
        return ClassificationResult {
            category: TrafficCategory::Images,
            reason: "Response MIME or path extension indicates an image asset".to_string(),
        };
    }

    if mime_lower.contains("font")
        || has_any_ext(&path_lower, &[".woff", ".woff2", ".ttf", ".otf", ".eot"])
    {
        return ClassificationResult {
            category: TrafficCategory::Fonts,
            reason: "Response MIME or path extension indicates a font file".to_string(),
        };
    }

    if mime_lower.contains("css") || path_lower.ends_with(".css") {
        return ClassificationResult {
            category: TrafficCategory::Css,
            reason: "Stylesheet asset detected".to_string(),
        };
    }

    if mime_lower.contains("javascript") || has_any_ext(&path_lower, &[".js", ".mjs"]) {
        return ClassificationResult {
            category: TrafficCategory::JavaScript,
            reason: "JavaScript bundle asset detected".to_string(),
        };
    }

    if mime_lower.starts_with("video/")
        || mime_lower.starts_with("audio/")
        || has_any_ext(&path_lower, &[".mp4", ".webm", ".mp3", ".m3u8"])
    {
        return ClassificationResult {
            category: TrafficCategory::Media,
            reason: "Streaming or media content detected".to_string(),
        };
    }

    if mime_lower.contains("wasm")
        || has_any_ext(&path_lower, &[".map", ".wasm", ".webmanifest"])
        || path_lower.ends_with("/manifest.json")
        || path_lower.starts_with("/static/")
        || path_lower.starts_with("/assets/")
    {
        return ClassificationResult {
            category: TrafficCategory::StaticAssets,
            reason: "Static asset bundle or source map detected".to_string(),
        };
    }

    if mime_lower.contains("html")
        || mime_lower.contains("pdf")
        || has_any_ext(&path_lower, &[".html", ".htm", ".pdf"])
    {
        return ClassificationResult {
            category: TrafficCategory::Documents,
            reason: "HTML or PDF document response detected".to_string(),
        };
    }

    let accepts_json = entry.request_headers.iter().any(|(name, val)| {
        name.eq_ignore_ascii_case("accept") && val.to_ascii_lowercase().contains("application/json")
    });
    let is_mutating_api = matches!(method_upper.as_str(), "POST" | "PUT" | "PATCH" | "DELETE");

    if mime_lower.contains("json")
        || entry.response_body_json.is_some()
        || entry.request_body_json.is_some()
        || accepts_json
        || path_lower.contains("/graphql")
        || has_api_version_prefix(&path_lower)
        || is_mutating_api
    {
        return ClassificationResult {
            category: TrafficCategory::Api,
            reason: "JSON payload, API route prefix, or mutating API request detected".to_string(),
        };
    }

    ClassificationResult {
        category: TrafficCategory::Other,
        reason: "No specialized asset or API signature matched".to_string(),
    }
}

/// Evaluates whether an entry is logically kept or excluded under `config`, returning the explainable reason.
#[must_use]
pub fn evaluate_entry(entry: &NormalizedEntry, config: &CleanerConfig) -> EntryFilterDecision {
    let base = classify_entry(entry);
    let mut category = base.category;
    let mut classification_reason = base.reason;

    if config.keep_request_ids.contains(&entry.id) {
        return EntryFilterDecision {
            entry_id: entry.id.clone(),
            session_id: entry.session_id.clone(),
            method: entry.method.clone(),
            host: entry.host.clone(),
            path: entry.path.clone(),
            category,
            classification_reason,
            kept: true,
            filter_reason: "Explicitly kept/restored by request ID".to_string(),
        };
    }

    if config.exclude_request_ids.contains(&entry.id) {
        return EntryFilterDecision {
            entry_id: entry.id.clone(),
            session_id: entry.session_id.clone(),
            method: entry.method.clone(),
            host: entry.host.clone(),
            path: entry.path.clone(),
            category,
            classification_reason,
            kept: false,
            filter_reason: "Explicitly excluded by request ID".to_string(),
        };
    }

    for rule in &config.custom_rules {
        if rule_matches(rule, entry) {
            if let Some(override_cat) = rule.override_category {
                category = override_cat;
                classification_reason = format!("Category overridden by custom rule '{}'", rule.id);
            }
            let kept = rule.action == FilterAction::Keep;
            return EntryFilterDecision {
                entry_id: entry.id.clone(),
                session_id: entry.session_id.clone(),
                method: entry.method.clone(),
                host: entry.host.clone(),
                path: entry.path.clone(),
                category,
                classification_reason,
                kept,
                filter_reason: format!("Matched custom rule '{}'", rule.id),
            };
        }
    }

    if config
        .excluded_domains
        .iter()
        .any(|dom| domain_matches(&entry.host, dom))
    {
        return EntryFilterDecision {
            entry_id: entry.id.clone(),
            session_id: entry.session_id.clone(),
            method: entry.method.clone(),
            host: entry.host.clone(),
            path: entry.path.clone(),
            category,
            classification_reason,
            kept: false,
            filter_reason: format!("Domain '{}' is excluded", entry.host),
        };
    }

    if config
        .included_domains
        .iter()
        .any(|dom| domain_matches(&entry.host, dom))
    {
        let is_cors_preflight = classification_reason.contains("CORS preflight");
        let is_static_noise = matches!(
            category,
            TrafficCategory::Images
                | TrafficCategory::Fonts
                | TrafficCategory::Css
                | TrafficCategory::JavaScript
                | TrafficCategory::Media
                | TrafficCategory::StaticAssets
        );
        if !is_cors_preflight && !is_static_noise {
            return EntryFilterDecision {
                entry_id: entry.id.clone(),
                session_id: entry.session_id.clone(),
                method: entry.method.clone(),
                host: entry.host.clone(),
                path: entry.path.clone(),
                category,
                classification_reason,
                kept: true,
                filter_reason: format!("Domain '{}' is explicitly included", entry.host),
            };
        }
    }

    let kept = config.keep_categories.contains(&category);
    let filter_reason = if kept {
        format!("Kept category '{}'", category.as_str())
    } else {
        format!("Excluded noise category '{}'", category.as_str())
    };

    EntryFilterDecision {
        entry_id: entry.id.clone(),
        session_id: entry.session_id.clone(),
        method: entry.method.clone(),
        host: entry.host.clone(),
        path: entry.path.clone(),
        category,
        classification_reason,
        kept,
        filter_reason,
    }
}

/// Computes a non-destructive preview of how `config` classifies and filters `sessions`.
#[must_use]
pub fn preview_cleaning(sessions: &[NormalizedSession], config: &CleanerConfig) -> CleanerPreview {
    let mut preview = CleanerPreview::default();
    for session in sessions {
        for entry in &session.entries {
            let decision = evaluate_entry(entry, config);
            preview.total_entries += 1;
            if decision.kept {
                preview.kept_entries += 1;
            } else {
                preview.excluded_entries += 1;
            }
            *preview
                .category_counts
                .entry(decision.category.as_str().to_string())
                .or_insert(0) += 1;
            preview.decisions.push(decision);
        }
    }
    preview
}

/// Produces a logically filtered copy of `sessions` alongside the preview report,
/// leaving the source sessions untouched for instant restoration.
#[must_use]
pub fn filter_sessions(
    sessions: &[NormalizedSession],
    config: &CleanerConfig,
) -> (Vec<NormalizedSession>, CleanerPreview) {
    let mut preview = CleanerPreview::default();
    let mut filtered_sessions = Vec::with_capacity(sessions.len());

    for session in sessions {
        let mut kept_entries = Vec::new();
        for entry in &session.entries {
            let decision = evaluate_entry(entry, config);
            preview.total_entries += 1;
            *preview
                .category_counts
                .entry(decision.category.as_str().to_string())
                .or_insert(0) += 1;
            if decision.kept {
                preview.kept_entries += 1;
                kept_entries.push(entry.clone());
            } else {
                preview.excluded_entries += 1;
            }
            preview.decisions.push(decision);
        }

        filtered_sessions.push(NormalizedSession {
            session_id: session.session_id.clone(),
            source_name: session.source_name.clone(),
            har_version: session.har_version.clone(),
            entries: kept_entries,
            redacted_secrets_count: session.redacted_secrets_count,
        });
    }

    (filtered_sessions, preview)
}

fn rule_matches(rule: &CustomCleanerRule, entry: &NormalizedEntry) -> bool {
    if let Some(ref dom) = rule.domain_suffix {
        if !domain_matches(&entry.host, dom) {
            return false;
        }
    }
    if let Some(ref prefix) = rule.path_prefix {
        if !entry
            .path
            .to_ascii_lowercase()
            .starts_with(&prefix.to_ascii_lowercase())
        {
            return false;
        }
    }
    if let Some(ref method) = rule.method {
        if !entry.method.eq_ignore_ascii_case(method.trim()) {
            return false;
        }
    }
    true
}

fn domain_matches(host: &str, pattern: &str) -> bool {
    let h = host.trim().to_ascii_lowercase();
    let p = pattern
        .trim()
        .trim_start_matches("*.")
        .trim_start_matches('.')
        .to_ascii_lowercase();
    if p.is_empty() {
        return false;
    }
    h == p || h.ends_with(&format!(".{p}"))
}

fn has_any_ext(path: &str, extensions: &[&str]) -> bool {
    extensions.iter().any(|ext| path.ends_with(ext))
}

fn path_contains_any(path: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| path.contains(n))
}

fn host_contains_any(host: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| host.contains(n))
}

fn has_api_version_prefix(path: &str) -> bool {
    if path.starts_with("/api/") {
        return true;
    }
    let trimmed = path.strip_prefix('/').unwrap_or(path);
    if let Some(first) = trimmed.split('/').next() {
        if let Some(rest) = first.strip_prefix('v') {
            return !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit());
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stub_entry(host: &str, path: &str, mime: &str) -> NormalizedEntry {
        NormalizedEntry {
            id: "req-1".into(),
            session_id: "sess-1".into(),
            started_date_time: "2026-11-18T00:00:00Z".into(),
            duration_ms: 10.0,
            method: "GET".into(),
            url: format!("https://{host}{path}"),
            host: host.into(),
            path: path.into(),
            response_status: 200,
            response_mime_type: mime.into(),
            ..NormalizedEntry::default()
        }
    }

    #[test]
    fn classifies_all_thirteen_categories_and_cors_preflight() {
        assert_eq!(
            classify_entry(&stub_entry(
                "api.example.test",
                "/oauth/token",
                "application/json"
            ))
            .category,
            TrafficCategory::Authentication
        );
        assert_eq!(
            classify_entry(&stub_entry(
                "api.example.test",
                "/api/orders",
                "application/json"
            ))
            .category,
            TrafficCategory::Api
        );
        assert_eq!(
            classify_entry(&stub_entry("cdn.example.test", "/logo.png", "image/png")).category,
            TrafficCategory::Images
        );
        assert_eq!(
            classify_entry(&stub_entry(
                "cdn.example.test",
                "/app.js",
                "application/javascript"
            ))
            .category,
            TrafficCategory::JavaScript
        );
        assert_eq!(
            classify_entry(&stub_entry("cdn.example.test", "/styles.css", "text/css")).category,
            TrafficCategory::Css
        );
        assert_eq!(
            classify_entry(&stub_entry(
                "cdn.example.test",
                "/inter.woff2",
                "font/woff2"
            ))
            .category,
            TrafficCategory::Fonts
        );
        assert_eq!(
            classify_entry(&stub_entry("cdn.example.test", "/clip.mp4", "video/mp4")).category,
            TrafficCategory::Media
        );
        assert_eq!(
            classify_entry(&stub_entry(
                "cdn.example.test",
                "/app.wasm",
                "application/wasm"
            ))
            .category,
            TrafficCategory::StaticAssets
        );
        assert_eq!(
            classify_entry(&stub_entry("docs.example.test", "/guide.html", "text/html")).category,
            TrafficCategory::Documents
        );
        assert_eq!(
            classify_entry(&stub_entry(
                "www.google-analytics.com",
                "/g/collect",
                "text/plain"
            ))
            .category,
            TrafficCategory::Analytics
        );
        assert_eq!(
            classify_entry(&stub_entry(
                "pagead.googlesyndication.com",
                "/ads/banner",
                "text/plain"
            ))
            .category,
            TrafficCategory::Advertising
        );
        assert_eq!(
            classify_entry(&stub_entry(
                "ingest.sentry.io",
                "/api/1/envelope",
                "application/json"
            ))
            .category,
            TrafficCategory::Telemetry
        );

        let mut preflight = stub_entry("api.example.test", "/v1/orders", "");
        preflight.method = "OPTIONS".into();
        preflight.request_headers = vec![("Access-Control-Request-Method".into(), "POST".into())];
        let res = classify_entry(&preflight);
        assert_eq!(res.category, TrafficCategory::Other);
        assert!(res.reason.contains("CORS"));
    }

    #[test]
    fn logical_filtering_supports_domain_custom_rules_and_restore() {
        let e1 = NormalizedEntry {
            id: "r1".into(),
            ..stub_entry("api.example.test", "/v1/users", "application/json")
        };
        let e2 = NormalizedEntry {
            id: "r2".into(),
            ..stub_entry("metrics.example.test", "/telemetry", "application/json")
        };
        let e3 = NormalizedEntry {
            id: "r3".into(),
            ..stub_entry("cdn.example.test", "/bundle.js", "application/javascript")
        };
        let e4 = NormalizedEntry {
            id: "r4".into(),
            ..stub_entry("legacy.example.test", "/custom-rpc", "text/plain")
        };

        let sessions = vec![NormalizedSession {
            session_id: "s1".into(),
            source_name: "demo.har".into(),
            har_version: "1.2".into(),
            entries: vec![e1, e2, e3, e4],
            redacted_secrets_count: 0,
        }];

        let mut config = CleanerConfig::default();
        config
            .excluded_domains
            .insert("metrics.example.test".into());
        config.custom_rules.push(CustomCleanerRule {
            id: "keep-rpc".into(),
            domain_suffix: Some("legacy.example.test".into()),
            path_prefix: Some("/custom-rpc".into()),
            method: Some("GET".into()),
            action: FilterAction::Keep,
            override_category: Some(TrafficCategory::Api),
        });

        let (filtered, preview) = filter_sessions(&sessions, &config);
        assert_eq!(preview.total_entries, 4);
        assert_eq!(preview.kept_entries, 2);
        assert_eq!(preview.excluded_entries, 2);
        assert_eq!(filtered[0].entries.len(), 2);
        assert_eq!(filtered[0].entries[1].id, "r4");

        // Exclude r1 logically, then restore r3 explicitly; original session remains 4 entries.
        config.exclude_request_ids.insert("r1".into());
        config.keep_request_ids.insert("r3".into());
        let (filtered_2, preview_2) = filter_sessions(&sessions, &config);
        assert_eq!(preview_2.kept_entries, 2);
        assert_eq!(filtered_2[0].entries[0].id, "r3");
        assert_eq!(sessions[0].entries.len(), 4);
    }
}
