use harflow_normalizer::NormalizedEntry;
use serde::{Deserialize, Serialize};

/// All 13 traffic classification categories specified in SRS M04.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

/// Classifies a normalized entry using deterministic MIME, path, and host signals.
#[must_use]
pub fn classify_entry(entry: &NormalizedEntry) -> ClassificationResult {
    let path_lower = entry.path.to_ascii_lowercase();
    let mime_lower = entry.response_mime_type.to_ascii_lowercase();
    let host_lower = entry.host.to_ascii_lowercase();

    if path_lower.contains("/auth")
        || path_lower.contains("/login")
        || path_lower.contains("/oauth")
        || path_lower.ends_with("/token")
        || path_lower.ends_with("/refresh")
    {
        return ClassificationResult {
            category: TrafficCategory::Authentication,
            reason: "Path matches authentication or token lifecycle keywords".to_string(),
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

    if mime_lower.starts_with("video/") || mime_lower.starts_with("audio/") {
        return ClassificationResult {
            category: TrafficCategory::Media,
            reason: "Streaming or media content detected".to_string(),
        };
    }

    if host_lower.contains("analytics") || path_lower.contains("/analytics") {
        return ClassificationResult {
            category: TrafficCategory::Analytics,
            reason: "Host or path matches analytics endpoint".to_string(),
        };
    }

    if path_lower.contains("/telemetry") || path_lower.contains("/metrics") {
        return ClassificationResult {
            category: TrafficCategory::Telemetry,
            reason: "Path matches telemetry or metrics endpoint".to_string(),
        };
    }

    if mime_lower.contains("json")
        || path_lower.starts_with("/api/")
        || path_lower.starts_with("/v1/")
        || path_lower.starts_with("/v2/")
        || entry.response_body_json.is_some()
    {
        return ClassificationResult {
            category: TrafficCategory::Api,
            reason: "JSON payload or API route prefix detected".to_string(),
        };
    }

    if mime_lower.contains("html") {
        return ClassificationResult {
            category: TrafficCategory::Documents,
            reason: "HTML document response detected".to_string(),
        };
    }

    ClassificationResult {
        category: TrafficCategory::Other,
        reason: "No specialized asset or API signature matched".to_string(),
    }
}

fn has_any_ext(path: &str, extensions: &[&str]) -> bool {
    extensions.iter().any(|ext| path.ends_with(ext))
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
            query_params: Vec::new(),
            request_headers: Vec::new(),
            request_cookies: Vec::new(),
            request_body: None,
            request_body_json: None,
            response_status: 200,
            response_headers: Vec::new(),
            response_cookies: Vec::new(),
            response_body: None,
            response_body_json: None,
            response_mime_type: mime.into(),
            response_encoding: None,
            transfer_size: 0,
        }
    }

    #[test]
    fn classifies_api_auth_and_static_assets() {
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
    }
}
