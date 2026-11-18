use harflow_model::{AuthFlow, AuthScheme};
use harflow_normalizer::NormalizedSession;
use serde::{Deserialize, Serialize};

/// Summary of observed authentication signals without retaining raw credential values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSchemeSummary {
    pub scheme: AuthScheme,
    pub observed_count: usize,
}

/// Detects observed Bearer Authorization, Cookie session, and CSRF mechanisms across sessions.
#[must_use]
pub fn detect_observed_auth(sessions: &[NormalizedSession]) -> Vec<AuthFlow> {
    let mut bearer_ids = Vec::new();
    let mut cookie_ids = Vec::new();
    let mut csrf_ids = Vec::new();

    for session in sessions {
        for entry in &session.entries {
            let has_bearer = entry.request_headers.iter().any(|(name, val)| {
                name.eq_ignore_ascii_case("authorization")
                    && val.to_ascii_lowercase().contains("authorization")
                    || name.eq_ignore_ascii_case("authorization")
                        && val.to_ascii_lowercase().contains("bearer")
            });
            if has_bearer {
                bearer_ids.push(entry.id.clone());
            }

            let has_cookie = !entry.request_cookies.is_empty()
                || entry
                    .request_headers
                    .iter()
                    .any(|(name, _)| name.eq_ignore_ascii_case("cookie"))
                || entry
                    .response_headers
                    .iter()
                    .any(|(name, _)| name.eq_ignore_ascii_case("set-cookie"));
            if has_cookie {
                cookie_ids.push(entry.id.clone());
            }

            let has_csrf = entry.request_headers.iter().any(|(name, _)| {
                let lower = name.to_ascii_lowercase();
                lower == "x-csrf-token" || lower == "x-xsrf-token"
            });
            if has_csrf {
                csrf_ids.push(entry.id.clone());
            }
        }
    }

    let mut flows = Vec::new();
    if !bearer_ids.is_empty() {
        flows.push(AuthFlow {
            id: "auth-bearer".to_string(),
            scheme: AuthScheme::Bearer,
            description: "Observed Authorization header (redacted before storage)".to_string(),
            request_ids: bearer_ids,
            confidence: 0.90,
        });
    }
    if !cookie_ids.is_empty() {
        flows.push(AuthFlow {
            id: "auth-cookie".to_string(),
            scheme: AuthScheme::CookieSession,
            description: "Observed Cookie / Set-Cookie session headers".to_string(),
            request_ids: cookie_ids,
            confidence: 0.85,
        });
    }
    if !csrf_ids.is_empty() {
        flows.push(AuthFlow {
            id: "auth-csrf".to_string(),
            scheme: AuthScheme::Csrf,
            description: "Observed CSRF header token usage".to_string(),
            request_ids: csrf_ids,
            confidence: 0.85,
        });
    }

    flows
}

#[cfg(test)]
mod tests {
    use harflow_har::{parse_har_bytes, ImportLimits};
    use harflow_normalizer::normalize_har_session;

    use super::*;

    #[test]
    fn detects_bearer_and_csrf_without_leaking_secrets() {
        let raw = br#"{
          "log": {
            "version": "1.2",
            "entries": [
              {
                "request": {
                  "method": "POST",
                  "url": "https://api.example.test/v1/orders",
                  "headers": [
                    { "name": "Authorization", "value": "Bearer hf_synthetic_secret_99" },
                    { "name": "X-CSRF-Token", "value": "hf_synthetic_csrf_99" }
                  ]
                },
                "response": { "status": 201 }
              }
            ]
          }
        }"#;

        let har = parse_har_bytes(raw, &ImportLimits::default()).expect("parse");
        let session = normalize_har_session("s1", "auth.har", &har);
        let flows = detect_observed_auth(&[session]);

        assert!(flows.iter().any(|f| f.scheme == AuthScheme::Bearer));
        assert!(flows.iter().any(|f| f.scheme == AuthScheme::Csrf));
        let serialized = serde_json::to_string(&flows).expect("json");
        assert!(!serialized.contains("hf_synthetic_secret_99"));
        assert!(!serialized.contains("hf_synthetic_csrf_99"));
    }
}
