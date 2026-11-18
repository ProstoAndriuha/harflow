use harflow_har::{HarEntry, HarFile};
use harflow_security::{
    sanitize_header_value, sanitize_json_value, sanitize_query_param, RedactedValue,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Normalized and secret-redacted representation of a single HTTP request/response pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedEntry {
    pub id: String,
    pub session_id: String,
    pub started_date_time: String,
    pub duration_ms: f64,
    pub method: String,
    pub url: String,
    pub host: String,
    pub path: String,
    pub query_params: Vec<(String, String)>,
    pub request_headers: Vec<(String, String)>,
    pub request_cookies: Vec<(String, String)>,
    pub request_body: Option<String>,
    pub request_body_json: Option<Value>,
    pub response_status: u16,
    pub response_headers: Vec<(String, String)>,
    pub response_cookies: Vec<(String, String)>,
    pub response_body: Option<String>,
    pub response_body_json: Option<Value>,
    pub response_mime_type: String,
    pub response_encoding: Option<String>,
    pub transfer_size: usize,
}

/// Normalized session produced from a single HAR file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedSession {
    pub session_id: String,
    pub source_name: String,
    pub har_version: String,
    pub entries: Vec<NormalizedEntry>,
    pub redacted_secrets_count: usize,
}

/// Normalizes all HAR entries in order while redacting sensitive headers, cookies, query params, and JSON payloads.
#[must_use]
pub fn normalize_har_session(
    session_id: &str,
    source_name: &str,
    har: &HarFile,
) -> NormalizedSession {
    let mut entries = Vec::with_capacity(har.log.entries.len());
    let mut total_redactions = 0usize;

    for (idx, raw) in har.log.entries.iter().enumerate() {
        let (entry, entry_redactions) = normalize_entry(session_id, idx, raw);
        total_redactions += entry_redactions;
        entries.push(entry);
    }

    NormalizedSession {
        session_id: session_id.to_string(),
        source_name: source_name.to_string(),
        har_version: har.log.version.clone(),
        entries,
        redacted_secrets_count: total_redactions,
    }
}

fn normalize_entry(session_id: &str, index: usize, raw: &HarEntry) -> (NormalizedEntry, usize) {
    let mut redactions = 0usize;
    let (host, path, fallback_query) = parse_url_parts(&raw.request.url);

    let raw_query = if raw.request.query_string.is_empty() {
        fallback_query
    } else {
        raw.request
            .query_string
            .iter()
            .map(|nv| (nv.name.clone(), nv.value.clone()))
            .collect()
    };

    let mut query_params = Vec::with_capacity(raw_query.len());
    for (k, v) in raw_query {
        let (clean, changed) = sanitize_query_param(&k, &v);
        if changed {
            redactions += 1;
        }
        query_params.push((k, clean));
    }

    let mut request_headers = Vec::with_capacity(raw.request.headers.len());
    for h in &raw.request.headers {
        let (clean, changed) = sanitize_header_value(&h.name, &h.value);
        if changed {
            redactions += 1;
        }
        request_headers.push((h.name.clone(), clean));
    }

    let mut request_cookies = Vec::with_capacity(raw.request.cookies.len());
    for c in &raw.request.cookies {
        let redacted = RedactedValue::from_secret("cookie", &c.value).marker();
        redactions += 1;
        request_cookies.push((c.name.clone(), redacted));
    }

    let (request_body, request_body_json, req_body_redactions) = raw
        .request
        .post_data
        .as_ref()
        .and_then(|pd| pd.text.as_deref())
        .map_or((None, None, 0), sanitize_body_text);
    redactions += req_body_redactions;

    let mut response_headers = Vec::with_capacity(raw.response.headers.len());
    for h in &raw.response.headers {
        let (clean, changed) = sanitize_header_value(&h.name, &h.value);
        if changed {
            redactions += 1;
        }
        response_headers.push((h.name.clone(), clean));
    }

    let mut response_cookies = Vec::with_capacity(raw.response.cookies.len());
    for c in &raw.response.cookies {
        let redacted = RedactedValue::from_secret("set_cookie", &c.value).marker();
        redactions += 1;
        response_cookies.push((c.name.clone(), redacted));
    }

    let (response_body, response_body_json, resp_body_redactions) = raw
        .response
        .content
        .text
        .as_deref()
        .map_or((None, None, 0), sanitize_body_text);
    redactions += resp_body_redactions;

    let transfer_size = raw
        .response
        .content
        .size
        .and_then(|s| usize::try_from(s).ok())
        .unwrap_or_else(|| raw.response.content.text.as_ref().map_or(0, String::len));

    let sanitized_url = rebuild_url_without_raw_secrets(&raw.request.url, &query_params);

    (
        NormalizedEntry {
            id: format!("{session_id}-req-{:04}", index + 1),
            session_id: session_id.to_string(),
            started_date_time: raw.started_date_time.clone(),
            duration_ms: raw.time.max(0.0),
            method: raw.request.method.trim().to_ascii_uppercase(),
            url: sanitized_url,
            host,
            path,
            query_params,
            request_headers,
            request_cookies,
            request_body,
            request_body_json,
            response_status: raw.response.status,
            response_headers,
            response_cookies,
            response_body,
            response_body_json,
            response_mime_type: raw.response.content.mime_type.clone(),
            response_encoding: raw.response.content.encoding.clone(),
            transfer_size,
        },
        redactions,
    )
}

fn sanitize_body_text(text: &str) -> (Option<String>, Option<Value>, usize) {
    if let Ok(parsed) = serde_json::from_str::<Value>(text) {
        let (clean_json, count) = sanitize_json_value(&parsed);
        let serialized = serde_json::to_string(&clean_json).ok();
        (serialized, Some(clean_json), count)
    } else {
        (Some(text.to_string()), None, 0)
    }
}

fn parse_url_parts(raw_url: &str) -> (String, String, Vec<(String, String)>) {
    let without_fragment = raw_url.split('#').next().unwrap_or(raw_url);
    let (base, query_str) = match without_fragment.split_once('?') {
        Some((b, q)) => (b, Some(q)),
        None => (without_fragment, None),
    };

    let after_scheme = base.split_once("://").map_or(base, |(_, rest)| rest);
    let (host_port, path) = match after_scheme.find('/') {
        Some(idx) => (&after_scheme[..idx], &after_scheme[idx..]),
        None => (after_scheme, "/"),
    };
    let host = host_port.split(':').next().unwrap_or(host_port).to_string();

    let query_params = query_str.map_or_else(Vec::new, |q| {
        q.split('&')
            .filter(|pair| !pair.is_empty())
            .map(|pair| match pair.split_once('=') {
                Some((k, v)) => (k.to_string(), v.to_string()),
                None => (pair.to_string(), String::new()),
            })
            .collect()
    });

    (host, path.to_string(), query_params)
}

fn rebuild_url_without_raw_secrets(raw_url: &str, sanitized_query: &[(String, String)]) -> String {
    let base = raw_url
        .split('#')
        .next()
        .unwrap_or(raw_url)
        .split('?')
        .next()
        .unwrap_or(raw_url);

    if sanitized_query.is_empty() {
        return base.to_string();
    }

    let pairs: Vec<String> = sanitized_query
        .iter()
        .map(|(k, v)| {
            if v.is_empty() {
                k.clone()
            } else {
                format!("{k}={v}")
            }
        })
        .collect();

    format!("{base}?{}", pairs.join("&"))
}

#[cfg(test)]
mod tests {
    use harflow_har::{parse_har_bytes, ImportLimits};

    use super::*;

    #[test]
    fn preserves_query_order_duplicate_headers_and_get_body_while_redacting_secrets() {
        let raw = br#"{
          "log": {
            "version": "1.2",
            "entries": [
              {
                "startedDateTime": "2026-11-18T00:00:00Z",
                "time": 12.5,
                "request": {
                  "method": "GET",
                  "url": "https://api.example.test/v1/items?tag=a&tag=b&api_key=hf_synthetic_key_1",
                  "headers": [
                    { "name": "X-Custom", "value": "first" },
                    { "name": "X-Custom", "value": "second" },
                    { "name": "Authorization", "value": "Bearer hf_synthetic_tok_1" }
                  ],
                  "queryString": [
                    { "name": "tag", "value": "a" },
                    { "name": "tag", "value": "b" },
                    { "name": "api_key", "value": "hf_synthetic_key_1" }
                  ],
                  "postData": { "mimeType": "application/json", "text": "{\"probe\":true}" }
                },
                "response": {
                  "status": 200,
                  "statusText": "OK",
                  "content": { "mimeType": "application/json", "text": "{\"ok\":true}" }
                }
              }
            ]
          }
        }"#;

        let har = parse_har_bytes(raw, &ImportLimits::default()).expect("parse");
        let normalized = normalize_har_session("sess-1", "test.har", &har);

        assert_eq!(normalized.entries.len(), 1);
        let entry = &normalized.entries[0];
        assert_eq!(entry.query_params[0], ("tag".into(), "a".into()));
        assert_eq!(entry.query_params[1], ("tag".into(), "b".into()));
        assert!(!entry.query_params[2].1.contains("hf_synthetic_key_1"));
        assert!(!entry.url.contains("hf_synthetic_key_1"));
        assert_eq!(entry.request_headers.len(), 3);
        assert!(!entry.request_headers[2].1.contains("hf_synthetic_tok_1"));
        assert_eq!(entry.request_body.as_deref(), Some("{\"probe\":true}"));
        assert_eq!(normalized.redacted_secrets_count, 2);
    }
}
