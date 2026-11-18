use std::io::Read;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use harflow_core::{FieldPresence, HarflowError};
use harflow_har::{stream_har_reader, HarEntry, HarFile, HarStreamSummary, ImportLimits};
use harflow_security::{
    sanitize_cookie_header, sanitize_form_urlencoded, sanitize_header_value, sanitize_json_value,
    sanitize_query_param, sanitize_secret_like_string, sanitize_set_cookie_header, RedactedValue,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Normalized and secret-redacted representation of a single HTTP request/response pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedEntry {
    pub id: String,
    pub session_id: String,
    pub started_date_time: String,
    pub duration_ms: f64,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub scheme: String,
    pub host: String,
    #[serde(default)]
    pub port: Option<u16>,
    pub path: String,
    pub query_params: Vec<(String, String)>,
    pub request_headers: Vec<(String, String)>,
    pub request_cookies: Vec<(String, String)>,
    #[serde(default)]
    pub request_mime_type: Option<String>,
    pub request_body: Option<String>,
    pub request_body_json: Option<Value>,
    #[serde(default)]
    pub request_body_presence: FieldPresence<String>,
    #[serde(default)]
    pub request_form_params: Vec<(String, String)>,
    pub response_status: u16,
    #[serde(default)]
    pub response_status_text: String,
    pub response_headers: Vec<(String, String)>,
    pub response_cookies: Vec<(String, String)>,
    pub response_body: Option<String>,
    pub response_body_json: Option<Value>,
    #[serde(default)]
    pub response_body_presence: FieldPresence<String>,
    pub response_mime_type: String,
    pub response_encoding: Option<String>,
    #[serde(default)]
    pub response_content_encoding: Option<String>,
    #[serde(default)]
    pub response_har_encoding: Option<String>,
    pub transfer_size: usize,
    #[serde(default)]
    pub initiator_type: Option<String>,
    #[serde(default)]
    pub initiator_url: Option<String>,
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

/// Normalizes an in-memory `HarFile` in order while redacting sensitive data.
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

/// Streams a HAR 1.2 or `.har.gz` reader and normalizes every entry on the fly.
pub fn normalize_har_reader<R: Read>(
    session_id: &str,
    source_name: &str,
    reader: R,
    limits: &ImportLimits,
) -> Result<(NormalizedSession, HarStreamSummary), HarflowError> {
    let mut entries = Vec::new();
    let mut total_redactions = 0usize;

    let summary = stream_har_reader(reader, limits, |idx, raw| {
        let (entry, entry_redactions) = normalize_entry(session_id, idx, &raw);
        total_redactions += entry_redactions;
        entries.push(entry);
        Ok(())
    })?;

    let session = NormalizedSession {
        session_id: session_id.to_string(),
        source_name: source_name.to_string(),
        har_version: summary.version.clone(),
        entries,
        redacted_secrets_count: total_redactions,
    };

    Ok((session, summary))
}

fn normalize_entry(session_id: &str, index: usize, raw: &HarEntry) -> (NormalizedEntry, usize) {
    let mut redactions = 0usize;
    let (scheme, host, port, path, fallback_query) = parse_url_parts(&raw.request.url);

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
    let mut header_cookies = Vec::new();
    for h in &raw.request.headers {
        if h.name.eq_ignore_ascii_case("cookie") {
            let (clean, pairs, count) = sanitize_cookie_header(&h.value);
            redactions += count;
            header_cookies.extend(pairs);
            request_headers.push((h.name.clone(), clean));
        } else {
            let (clean, changed) = sanitize_header_value(&h.name, &h.value);
            if changed {
                redactions += 1;
            }
            request_headers.push((h.name.clone(), clean));
        }
    }

    let mut request_cookies = Vec::with_capacity(raw.request.cookies.len());
    for c in &raw.request.cookies {
        let redacted = RedactedValue::from_secret("cookie", &c.value).marker();
        redactions += 1;
        request_cookies.push((c.name.clone(), redacted));
    }
    if request_cookies.is_empty() && !header_cookies.is_empty() {
        request_cookies = header_cookies;
    }

    let request_mime_type = raw
        .request
        .post_data
        .as_ref()
        .map(|pd| pd.mime_type.clone())
        .filter(|m| !m.is_empty());

    let NormalizedRequestBody {
        body: request_body,
        json: request_body_json,
        presence: request_body_presence,
        form_params: request_form_params,
        redactions: req_body_redactions,
    } = normalize_request_post_data(raw.request.post_data.as_ref());
    redactions += req_body_redactions;

    let mut response_headers = Vec::with_capacity(raw.response.headers.len());
    let mut header_set_cookies = Vec::new();
    let mut response_content_encoding = None;

    for h in &raw.response.headers {
        if h.name.eq_ignore_ascii_case("content-encoding") && response_content_encoding.is_none() {
            response_content_encoding = Some(h.value.trim().to_string());
        }

        if h.name.eq_ignore_ascii_case("set-cookie") {
            let (clean, maybe_pair, count) = sanitize_set_cookie_header(&h.value);
            redactions += count;
            if let Some(pair) = maybe_pair {
                header_set_cookies.push(pair);
            }
            response_headers.push((h.name.clone(), clean));
        } else {
            let (clean, changed) = sanitize_header_value(&h.name, &h.value);
            if changed {
                redactions += 1;
            }
            response_headers.push((h.name.clone(), clean));
        }
    }

    let mut response_cookies = Vec::with_capacity(raw.response.cookies.len());
    for c in &raw.response.cookies {
        let redacted = RedactedValue::from_secret("cookie", &c.value).marker();
        redactions += 1;
        response_cookies.push((c.name.clone(), redacted));
    }
    if response_cookies.is_empty() && !header_set_cookies.is_empty() {
        response_cookies = header_set_cookies;
    }

    let response_har_encoding = raw.response.content.encoding.clone();
    let (response_body, response_body_json, response_body_presence, resp_body_redactions) =
        normalize_response_content(
            raw.response.content.text.as_deref(),
            &raw.response.content.mime_type,
            response_har_encoding.as_deref(),
        );
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
            scheme,
            host,
            port,
            path,
            query_params,
            request_headers,
            request_cookies,
            request_mime_type,
            request_body,
            request_body_json,
            request_body_presence,
            request_form_params,
            response_status: raw.response.status,
            response_status_text: raw.response.status_text.clone(),
            response_headers,
            response_cookies,
            response_body,
            response_body_json,
            response_body_presence,
            response_mime_type: raw.response.content.mime_type.clone(),
            response_encoding: response_har_encoding.clone(),
            response_content_encoding,
            response_har_encoding,
            transfer_size,
            initiator_type: raw
                .initiator
                .as_ref()
                .map(|i| i.initiator_type.clone())
                .filter(|t| !t.is_empty()),
            initiator_url: raw.initiator.as_ref().and_then(|i| i.url.clone()),
        },
        redactions,
    )
}

struct NormalizedRequestBody {
    body: Option<String>,
    json: Option<Value>,
    presence: FieldPresence<String>,
    form_params: Vec<(String, String)>,
    redactions: usize,
}

fn normalize_request_post_data(
    post_data: Option<&harflow_har::HarPostData>,
) -> NormalizedRequestBody {
    let Some(pd) = post_data else {
        return NormalizedRequestBody {
            body: None,
            json: None,
            presence: FieldPresence::Missing,
            form_params: Vec::new(),
            redactions: 0,
        };
    };

    let Some(text) = pd.text.as_deref() else {
        return NormalizedRequestBody {
            body: None,
            json: None,
            presence: FieldPresence::Null,
            form_params: Vec::new(),
            redactions: 0,
        };
    };

    if pd
        .mime_type
        .to_ascii_lowercase()
        .contains("x-www-form-urlencoded")
    {
        let (clean_form, pairs, count) = sanitize_form_urlencoded(text);
        return NormalizedRequestBody {
            body: Some(clean_form.clone()),
            json: None,
            presence: FieldPresence::Value(clean_form),
            form_params: pairs,
            redactions: count,
        };
    }

    let (body, json_val, count) = sanitize_body_text(text);
    let presence = body
        .clone()
        .map_or(FieldPresence::Null, FieldPresence::Value);
    NormalizedRequestBody {
        body,
        json: json_val,
        presence,
        form_params: Vec::new(),
        redactions: count,
    }
}

fn normalize_response_content(
    raw_text: Option<&str>,
    mime_type: &str,
    har_encoding: Option<&str>,
) -> (Option<String>, Option<Value>, FieldPresence<String>, usize) {
    let Some(text) = raw_text else {
        return (None, None, FieldPresence::Missing, 0);
    };

    let is_base64 = har_encoding.is_some_and(|enc| enc.eq_ignore_ascii_case("base64"));
    let mime_lower = mime_type.to_ascii_lowercase();
    let is_textual_mime = mime_lower.contains("json")
        || mime_lower.starts_with("text/")
        || mime_lower.contains("xml")
        || mime_lower.contains("javascript")
        || mime_lower.contains("x-www-form-urlencoded");

    if is_base64 {
        if is_textual_mime {
            if let Ok(decoded_bytes) = STANDARD.decode(text.trim()) {
                if let Ok(decoded_utf8) = String::from_utf8(decoded_bytes) {
                    let (clean, json_val, count) = sanitize_body_text(&decoded_utf8);
                    let presence = clean
                        .clone()
                        .map_or(FieldPresence::Null, FieldPresence::Value);
                    return (clean, json_val, presence, count);
                }
            }
        }
        return (
            Some(text.to_string()),
            None,
            FieldPresence::Value(text.to_string()),
            0,
        );
    }

    let (clean, json_val, count) = sanitize_body_text(text);
    let presence = clean
        .clone()
        .map_or(FieldPresence::Null, FieldPresence::Value);
    (clean, json_val, presence, count)
}

fn sanitize_body_text(text: &str) -> (Option<String>, Option<Value>, usize) {
    if let Ok(parsed) = serde_json::from_str::<Value>(text) {
        let (clean_json, count) = sanitize_json_value(&parsed);
        let serialized = serde_json::to_string(&clean_json).ok();
        (serialized, Some(clean_json), count)
    } else if let Some(redacted) = sanitize_secret_like_string(text) {
        (Some(redacted), None, 1)
    } else {
        (Some(text.to_string()), None, 0)
    }
}

fn parse_url_parts(raw_url: &str) -> (String, String, Option<u16>, String, Vec<(String, String)>) {
    let without_fragment = raw_url.split('#').next().unwrap_or(raw_url);
    let (base, query_str) = match without_fragment.split_once('?') {
        Some((b, q)) => (b, Some(q)),
        None => (without_fragment, None),
    };

    let (scheme, after_scheme) = match base.split_once("://") {
        Some((s, rest)) => (s.to_ascii_lowercase(), rest),
        None => (String::new(), base),
    };
    let (host_port, path) = match after_scheme.find('/') {
        Some(idx) => (&after_scheme[..idx], &after_scheme[idx..]),
        None => (after_scheme, "/"),
    };
    let (host, port) = match host_port.rsplit_once(':') {
        Some((h, p)) if !h.contains(']') => match p.parse::<u16>() {
            Ok(port_num) => (h.to_string(), Some(port_num)),
            Err(_) => (host_port.to_string(), None),
        },
        _ => (host_port.to_string(), None),
    };

    let query_params = query_str.map_or_else(Vec::new, |q| {
        q.split('&')
            .filter(|pair| !pair.is_empty())
            .map(|pair| match pair.split_once('=') {
                Some((k, v)) => (k.to_string(), v.to_string()),
                None => (pair.to_string(), String::new()),
            })
            .collect()
    });

    (scheme, host, port, path.to_string(), query_params)
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
        assert_eq!(entry.scheme, "https");
        assert_eq!(entry.query_params[0], ("tag".into(), "a".into()));
        assert_eq!(entry.query_params[1], ("tag".into(), "b".into()));
        assert!(!entry.query_params[2].1.contains("hf_synthetic_key_1"));
        assert!(!entry.url.contains("hf_synthetic_key_1"));
        assert_eq!(entry.request_headers.len(), 3);
        assert!(!entry.request_headers[2].1.contains("hf_synthetic_tok_1"));
        assert_eq!(entry.request_body.as_deref(), Some("{\"probe\":true}"));
        assert_eq!(normalized.redacted_secrets_count, 2);
    }

    #[test]
    fn tracks_field_presence_encodings_form_bodies_and_header_cookies() {
        let json_payload = br#"{"access_token":"hf_synthetic_b64_secret","items":[1,2]}"#;
        let b64_response = STANDARD.encode(json_payload);

        let har_text = format!(
            r#"{{
              "log": {{
                "version": "1.2",
                "entries": [
                  {{
                    "request": {{
                      "method": "POST",
                      "url": "https://auth.example.test:8443/oauth/token",
                      "headers": [
                        {{ "name": "Cookie", "value": "sid=hf_synthetic_cookie_abc" }}
                      ],
                      "postData": {{
                        "mimeType": "application/x-www-form-urlencoded",
                        "text": "grant_type=client_credentials&client_secret=hf_synthetic_cs_123"
                      }}
                    }},
                    "response": {{
                      "status": 200,
                      "headers": [
                        {{ "name": "Content-Encoding", "value": "br" }},
                        {{ "name": "Set-Cookie", "value": "sid=hf_synthetic_cookie_abc; HttpOnly" }}
                      ],
                      "content": {{
                        "mimeType": "application/json",
                        "encoding": "base64",
                        "text": "{b64_response}"
                      }}
                    }}
                  }},
                  {{
                    "request": {{
                      "method": "GET",
                      "url": "https://api.example.test/ping"
                    }},
                    "response": {{ "status": 204, "content": {{ "text": "" }} }}
                  }},
                  {{
                    "request": {{
                      "method": "POST",
                      "url": "https://api.example.test/null-text",
                      "postData": {{ "mimeType": "application/json", "text": null }}
                    }},
                    "response": {{ "status": 200 }}
                  }}
                ]
              }}
            }}"#
        );

        let (normalized, summary) = normalize_har_reader(
            "s1",
            "encodings.har",
            har_text.as_bytes(),
            &ImportLimits::default(),
        )
        .expect("stream normalize");

        assert_eq!(summary.processed_entries, 3);
        let first = &normalized.entries[0];
        assert_eq!(first.port, Some(8443));
        assert_eq!(first.response_content_encoding.as_deref(), Some("br"));
        assert_eq!(first.response_har_encoding.as_deref(), Some("base64"));
        assert_eq!(first.request_cookies.len(), 1);
        assert_eq!(first.response_cookies.len(), 1);
        assert_eq!(first.request_cookies[0].1, first.response_cookies[0].1);
        assert_eq!(
            first.request_form_params[0],
            ("grant_type".into(), "client_credentials".into())
        );
        assert!(!first.request_form_params[1]
            .1
            .contains("hf_synthetic_cs_123"));
        let decoded_resp = first
            .response_body_json
            .as_ref()
            .expect("decoded base64 json");
        assert!(!decoded_resp.to_string().contains("hf_synthetic_b64_secret"));
        assert_eq!(decoded_resp["items"], serde_json::json!([1, 2]));

        let second = &normalized.entries[1];
        assert_eq!(second.request_body_presence, FieldPresence::Missing);
        assert_eq!(
            second.response_body_presence,
            FieldPresence::Value(String::new())
        );

        let third = &normalized.entries[2];
        assert_eq!(third.request_body_presence, FieldPresence::Null);
        assert_eq!(third.response_body_presence, FieldPresence::Missing);
    }
}
