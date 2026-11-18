use std::collections::HashMap;

use harflow_model::{Endpoint, ValueLocation};
use harflow_normalizer::{NormalizedEntry, NormalizedSession};
use harflow_security::{fingerprint_secret, parse_redacted_marker};
use serde_json::Value;

use crate::timeline::{build_session_timelines, SessionTimeline};

const MAX_JSON_EXTRACT_DEPTH: usize = 12;
const MAX_JSON_ARRAY_ITEMS: usize = 25;
const MAX_SCALAR_VALUE_LEN: usize = 4096;

const NOISE_RESPONSE_HEADERS: &[&str] = &[
    "date",
    "server",
    "content-type",
    "content-length",
    "connection",
    "vary",
    "cache-control",
    "strict-transport-security",
    "content-encoding",
    "transfer-encoding",
    "set-cookie",
    "keep-alive",
    "expires",
    "pragma",
];

const STATIC_REQUEST_HEADERS: &[&str] = &[
    "accept",
    "accept-encoding",
    "accept-language",
    "user-agent",
    "host",
    "origin",
    "referer",
    "connection",
    "content-type",
    "content-length",
    "cache-control",
    "pragma",
    "upgrade-insecure-requests",
    "dnt",
    "te",
    "cookie",
];

/// Coarse scalar type hint inferred during extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarTypeHint {
    Redacted,
    Uuid,
    Integer,
    Number,
    String,
}

/// Extracted candidate scalar value from either a producer response or consumer request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateValue {
    pub session_id: String,
    pub request_id: String,
    pub timeline_pos: usize,
    pub started_date_time: String,
    pub endpoint_id: Option<String>,
    pub endpoint_key: String,
    pub method: String,
    pub host: String,
    pub response_status: u16,
    pub location: ValueLocation,
    pub path_or_key: String,
    pub normalized_key: String,
    pub value: String,
    pub redacted_marker: Option<(String, String)>,
    pub plain_fingerprint: Option<String>,
    pub type_hint: ScalarTypeHint,
    pub derived_prefix_suffix: bool,
}

/// All producer and consumer candidates extracted for one entry on a session timeline.
#[derive(Debug, Clone)]
pub struct EntryCandidates {
    pub session_id: String,
    pub request_id: String,
    pub timeline_pos: usize,
    pub producers: Vec<CandidateValue>,
    pub consumers: Vec<CandidateValue>,
}

/// Extracts chronological producer & consumer candidates for every session.
#[must_use]
pub fn extract_candidates(
    sessions: &[NormalizedSession],
    endpoints: &[Endpoint],
) -> Vec<(String, Vec<EntryCandidates>)> {
    let timelines = build_session_timelines(sessions);
    timelines
        .iter()
        .map(|tl| {
            (
                tl.session_id.to_string(),
                extract_timeline_candidates(tl, endpoints),
            )
        })
        .collect()
}

/// Extracts all producer & consumer candidates from one chronological `SessionTimeline`.
#[must_use]
pub fn extract_timeline_candidates(
    timeline: &SessionTimeline<'_>,
    endpoints: &[Endpoint],
) -> Vec<EntryCandidates> {
    let matcher = EndpointPathMatcher::new(endpoints);
    let mut results = Vec::with_capacity(timeline.entries.len());

    for (timeline_pos, item) in timeline.entries.iter().enumerate() {
        let entry = item.entry;
        let (endpoint_id, endpoint_key, path_params) = matcher.match_entry(entry);

        let mut producers = Vec::new();
        let mut consumers = Vec::new();

        extract_entry_producers(
            entry,
            timeline_pos,
            endpoint_id.as_deref(),
            &endpoint_key,
            &mut producers,
        );
        extract_entry_consumers(
            entry,
            timeline_pos,
            endpoint_id.as_deref(),
            &endpoint_key,
            &path_params,
            &mut consumers,
        );

        results.push(EntryCandidates {
            session_id: entry.session_id.clone(),
            request_id: entry.id.clone(),
            timeline_pos,
            producers,
            consumers,
        });
    }

    results
}

struct EndpointSpec<'a> {
    id: &'a str,
    method: &'a str,
    host: &'a str,
    path_template: &'a str,
    template_segments: Vec<&'a str>,
    param_positions: Vec<(usize, String)>,
}

struct EndpointPathMatcher<'a> {
    specs: Vec<EndpointSpec<'a>>,
    by_request_id: HashMap<&'a str, usize>,
}

impl<'a> EndpointPathMatcher<'a> {
    fn new(endpoints: &'a [Endpoint]) -> Self {
        let mut specs = Vec::with_capacity(endpoints.len());
        let mut by_request_id = HashMap::new();

        for ep in endpoints {
            let template_segments: Vec<&str> = ep
                .path_template
                .split('/')
                .filter(|s| !s.is_empty())
                .collect();
            let param_positions = template_segments
                .iter()
                .enumerate()
                .filter_map(|(pos, seg)| {
                    seg.strip_prefix('{')
                        .and_then(|s| s.strip_suffix('}'))
                        .map(|name| (pos, name.to_string()))
                })
                .collect();

            let spec_idx = specs.len();
            specs.push(EndpointSpec {
                id: ep.id.as_str(),
                method: ep.method.as_str(),
                host: ep.host.as_str(),
                path_template: ep.path_template.as_str(),
                template_segments,
                param_positions,
            });
            for req_id in &ep.observed_request_ids {
                by_request_id.insert(req_id.as_str(), spec_idx);
            }
        }

        Self {
            specs,
            by_request_id,
        }
    }

    fn match_entry(
        &self,
        entry: &NormalizedEntry,
    ) -> (Option<String>, String, Vec<(String, String)>) {
        let path_segs: Vec<&str> = entry.path.split('/').filter(|s| !s.is_empty()).collect();

        if let Some(&idx) = self.by_request_id.get(entry.id.as_str()) {
            let spec = &self.specs[idx];
            let params = extract_params_from_spec(spec, &path_segs);
            return (
                Some(spec.id.to_string()),
                format!("{} {} {}", spec.method, spec.host, spec.path_template),
                params,
            );
        }

        for spec in &self.specs {
            if spec.method == entry.method
                && spec.host == entry.host
                && spec.template_segments.len() == path_segs.len()
                && spec
                    .template_segments
                    .iter()
                    .zip(path_segs.iter())
                    .all(|(tpl, raw)| tpl.starts_with('{') || *tpl == *raw)
            {
                let params = extract_params_from_spec(spec, &path_segs);
                return (
                    Some(spec.id.to_string()),
                    format!("{} {} {}", spec.method, spec.host, spec.path_template),
                    params,
                );
            }
        }

        let fallback_params = extract_fallback_path_params(&path_segs);
        let fallback_key = format!("{} {} {}", entry.method, entry.host, entry.path);
        (None, fallback_key, fallback_params)
    }
}

fn extract_params_from_spec(spec: &EndpointSpec<'_>, path_segs: &[&str]) -> Vec<(String, String)> {
    spec.param_positions
        .iter()
        .filter_map(|(pos, name)| {
            path_segs
                .get(*pos)
                .map(|raw| (name.clone(), (*raw).to_string()))
        })
        .collect()
}

fn extract_fallback_path_params(path_segs: &[&str]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (pos, seg) in path_segs.iter().enumerate() {
        if looks_like_dynamic_segment(pos, path_segs.get(pos.saturating_sub(1)).copied(), seg) {
            let name = path_segs
                .get(pos.saturating_sub(1))
                .filter(|_| pos > 0)
                .map_or_else(|| "id".to_string(), |prev| singularize_param(prev));
            out.push((name, (*seg).to_string()));
        }
    }
    out
}

fn looks_like_dynamic_segment(pos: usize, prev: Option<&str>, seg: &str) -> bool {
    if is_uuid_like(seg) {
        return true;
    }
    if !seg.is_empty() && seg.bytes().all(|b| b.is_ascii_digit()) {
        if let Ok(num) = seg.parse::<u64>() {
            let is_version_pos = pos <= 1 || prev.is_some_and(|p| p.eq_ignore_ascii_case("api"));
            if num <= 9 && is_version_pos {
                return false;
            }
        }
        return true;
    }
    seg.len() >= 6
        && (seg.contains('_') || seg.contains('-'))
        && seg.bytes().any(|b| b.is_ascii_digit())
}

fn singularize_param(prev: &str) -> String {
    let lower = prev.to_ascii_lowercase();
    let base = if lower.ends_with("ies") && lower.len() > 4 {
        format!("{}y", &lower[..lower.len() - 3])
    } else if lower.ends_with('s') && !lower.ends_with("ss") && lower.len() > 2 {
        lower[..lower.len() - 1].to_string()
    } else {
        lower
    };
    format!("{base}_id")
}

fn extract_entry_producers(
    entry: &NormalizedEntry,
    timeline_pos: usize,
    endpoint_id: Option<&str>,
    endpoint_key: &str,
    out: &mut Vec<CandidateValue>,
) {
    let allow_set_cookie =
        (200..=399).contains(&entry.response_status) || matches!(entry.response_status, 401 | 403);
    if allow_set_cookie {
        for (name, val) in &entry.response_cookies {
            push_candidate(
                out,
                entry,
                timeline_pos,
                endpoint_id,
                endpoint_key,
                ValueLocation::SetCookie,
                name.clone(),
                name.to_ascii_lowercase(),
                val.clone(),
                false,
            );
        }
    }

    if !(200..=399).contains(&entry.response_status) {
        return;
    }

    for (name, val) in &entry.response_headers {
        let lower = name.trim().to_ascii_lowercase();
        if NOISE_RESPONSE_HEADERS.contains(&lower.as_str()) || lower.starts_with("access-control-")
        {
            continue;
        }
        push_candidate(
            out,
            entry,
            timeline_pos,
            endpoint_id,
            endpoint_key,
            ValueLocation::ResponseHeader,
            name.clone(),
            lower.clone(),
            val.clone(),
            false,
        );

        if lower == "location" {
            if let Some(trailing) = trailing_url_segment(val) {
                push_candidate(
                    out,
                    entry,
                    timeline_pos,
                    endpoint_id,
                    endpoint_key,
                    ValueLocation::ResponseHeader,
                    name.clone(),
                    "location#segment".to_string(),
                    trailing,
                    true,
                );
            }
        }
    }

    if let Some(json) = &entry.response_body_json {
        extract_json_scalars(
            json,
            "$",
            "$",
            0,
            entry,
            timeline_pos,
            endpoint_id,
            endpoint_key,
            ValueLocation::ResponseBody,
            out,
        );
    }
}

fn extract_entry_consumers(
    entry: &NormalizedEntry,
    timeline_pos: usize,
    endpoint_id: Option<&str>,
    endpoint_key: &str,
    path_params: &[(String, String)],
    out: &mut Vec<CandidateValue>,
) {
    for (name, val) in path_params {
        push_candidate(
            out,
            entry,
            timeline_pos,
            endpoint_id,
            endpoint_key,
            ValueLocation::Path,
            name.clone(),
            name.to_ascii_lowercase(),
            val.clone(),
            false,
        );
    }

    for (name, val) in &entry.query_params {
        push_candidate(
            out,
            entry,
            timeline_pos,
            endpoint_id,
            endpoint_key,
            ValueLocation::Query,
            name.clone(),
            name.to_ascii_lowercase(),
            val.clone(),
            false,
        );
    }

    for (name, val) in &entry.request_headers {
        let lower = name.trim().to_ascii_lowercase();
        if STATIC_REQUEST_HEADERS.contains(&lower.as_str()) || lower.starts_with("sec-") {
            continue;
        }
        push_candidate(
            out,
            entry,
            timeline_pos,
            endpoint_id,
            endpoint_key,
            ValueLocation::Header,
            name.clone(),
            lower,
            val.clone(),
            false,
        );
    }

    for (name, val) in &entry.request_cookies {
        push_candidate(
            out,
            entry,
            timeline_pos,
            endpoint_id,
            endpoint_key,
            ValueLocation::Cookie,
            name.clone(),
            name.to_ascii_lowercase(),
            val.clone(),
            false,
        );
    }

    if let Some(json) = &entry.request_body_json {
        extract_json_scalars(
            json,
            "$",
            "$",
            0,
            entry,
            timeline_pos,
            endpoint_id,
            endpoint_key,
            ValueLocation::Body,
            out,
        );
    }

    for (name, val) in &entry.request_form_params {
        push_candidate(
            out,
            entry,
            timeline_pos,
            endpoint_id,
            endpoint_key,
            ValueLocation::Body,
            name.clone(),
            name.to_ascii_lowercase(),
            val.clone(),
            false,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn extract_json_scalars(
    value: &Value,
    concrete_path: &str,
    norm_path: &str,
    depth: usize,
    entry: &NormalizedEntry,
    timeline_pos: usize,
    endpoint_id: Option<&str>,
    endpoint_key: &str,
    location: ValueLocation,
    out: &mut Vec<CandidateValue>,
) {
    if depth > MAX_JSON_EXTRACT_DEPTH {
        return;
    }

    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let child_concrete = format!("{concrete_path}.{k}");
                let child_norm = format!("{norm_path}.{k}");
                extract_json_scalars(
                    v,
                    &child_concrete,
                    &child_norm,
                    depth + 1,
                    entry,
                    timeline_pos,
                    endpoint_id,
                    endpoint_key,
                    location.clone(),
                    out,
                );
            }
        }
        Value::Array(items) => {
            for (idx, item) in items.iter().take(MAX_JSON_ARRAY_ITEMS).enumerate() {
                let child_concrete = format!("{concrete_path}[{idx}]");
                let child_norm = format!("{norm_path}[*]");
                extract_json_scalars(
                    item,
                    &child_concrete,
                    &child_norm,
                    depth + 1,
                    entry,
                    timeline_pos,
                    endpoint_id,
                    endpoint_key,
                    location.clone(),
                    out,
                );
            }
        }
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() || trimmed.len() > MAX_SCALAR_VALUE_LEN {
                return;
            }
            push_candidate(
                out,
                entry,
                timeline_pos,
                endpoint_id,
                endpoint_key,
                location,
                concrete_path.to_string(),
                norm_path.to_ascii_lowercase(),
                trimmed.to_string(),
                false,
            );
        }
        Value::Number(n) => {
            push_candidate_with_hint(
                out,
                entry,
                timeline_pos,
                endpoint_id,
                endpoint_key,
                location,
                concrete_path.to_string(),
                norm_path.to_ascii_lowercase(),
                n.to_string(),
                if n.is_i64() || n.is_u64() {
                    ScalarTypeHint::Integer
                } else {
                    ScalarTypeHint::Number
                },
                false,
            );
        }
        Value::Bool(_) | Value::Null => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn push_candidate(
    out: &mut Vec<CandidateValue>,
    entry: &NormalizedEntry,
    timeline_pos: usize,
    endpoint_id: Option<&str>,
    endpoint_key: &str,
    location: ValueLocation,
    path_or_key: String,
    normalized_key: String,
    value: String,
    derived_prefix_suffix: bool,
) {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_SCALAR_VALUE_LEN {
        return;
    }
    let hint = classify_string_hint(trimmed);
    push_candidate_with_hint(
        out,
        entry,
        timeline_pos,
        endpoint_id,
        endpoint_key,
        location,
        path_or_key,
        normalized_key,
        trimmed.to_string(),
        hint,
        derived_prefix_suffix,
    );
}

#[allow(clippy::too_many_arguments)]
fn push_candidate_with_hint(
    out: &mut Vec<CandidateValue>,
    entry: &NormalizedEntry,
    timeline_pos: usize,
    endpoint_id: Option<&str>,
    endpoint_key: &str,
    location: ValueLocation,
    path_or_key: String,
    normalized_key: String,
    value: String,
    type_hint: ScalarTypeHint,
    derived_prefix_suffix: bool,
) {
    let redacted_marker =
        parse_redacted_marker(&value).map(|(kind, fp)| (kind.to_string(), fp.to_string()));
    let plain_fingerprint = if redacted_marker.is_none() && value.len() >= 6 {
        Some(fingerprint_secret(&value))
    } else {
        None
    };

    out.push(CandidateValue {
        session_id: entry.session_id.clone(),
        request_id: entry.id.clone(),
        timeline_pos,
        started_date_time: entry.started_date_time.clone(),
        endpoint_id: endpoint_id.map(ToString::to_string),
        endpoint_key: endpoint_key.to_string(),
        method: entry.method.clone(),
        host: entry.host.clone(),
        response_status: entry.response_status,
        location,
        path_or_key,
        normalized_key,
        value,
        redacted_marker,
        plain_fingerprint,
        type_hint,
        derived_prefix_suffix,
    });
}

fn classify_string_hint(val: &str) -> ScalarTypeHint {
    if parse_redacted_marker(val).is_some() {
        ScalarTypeHint::Redacted
    } else if is_uuid_like(val) {
        ScalarTypeHint::Uuid
    } else if val.bytes().all(|b| b.is_ascii_digit()) {
        ScalarTypeHint::Integer
    } else {
        ScalarTypeHint::String
    }
}

fn is_uuid_like(val: &str) -> bool {
    let b = val.as_bytes();
    b.len() == 36
        && b[8] == b'-'
        && b[13] == b'-'
        && b[18] == b'-'
        && b[23] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| matches!(i, 8 | 13 | 18 | 23) || c.is_ascii_hexdigit())
}

fn trailing_url_segment(raw: &str) -> Option<String> {
    let no_query = raw.trim().split(['?', '#']).next()?;
    let trimmed = no_query.trim_end_matches('/');
    let (_, seg) = trimmed.rsplit_once('/')?;
    let clean = seg.trim();
    if clean.len() < 2 || clean == trimmed {
        return None;
    }
    Some(clean.to_string())
}

#[cfg(test)]
mod tests {
    use harflow_endpoints::discover_endpoints;
    use harflow_har::{parse_har_bytes, ImportLimits};
    use harflow_normalizer::normalize_har_session;

    use super::*;

    #[test]
    fn extracts_all_eight_value_locations_with_deterministic_locators() {
        let raw = br#"{
          "log": {
            "version": "1.2",
            "entries": [
              {
                "startedDateTime": "2026-11-18T02:00:00Z",
                "time": 12.0,
                "request": {
                  "method": "POST",
                  "url": "https://api.example.test/v1/sessions",
                  "postData": { "mimeType": "application/x-www-form-urlencoded", "text": "grant_type=client_credentials" }
                },
                "response": {
                  "status": 201,
                  "headers": [
                    { "name": "Set-Cookie", "value": "sid=hf_synthetic_sid_9; HttpOnly" },
                    { "name": "X-Request-Id", "value": "req_9001" },
                    { "name": "Location", "value": "https://api.example.test/v1/orders/ord_777" },
                    { "name": "Date", "value": "Wed, 18 Nov 2026 02:00:00 GMT" }
                  ],
                  "content": {
                    "mimeType": "application/json",
                    "text": "{\"orderId\":\"ord_777\",\"access_token\":\"hf_synthetic_tok_77\",\"items\":[{\"id\":\"item_1\"}]}"
                  }
                }
              },
              {
                "startedDateTime": "2026-11-18T02:00:01Z",
                "time": 10.0,
                "request": {
                  "method": "POST",
                  "url": "https://api.example.test/v1/orders/ord_777?trace=req_9001",
                  "headers": [
                    { "name": "Authorization", "value": "Bearer hf_synthetic_tok_77" },
                    { "name": "Cookie", "value": "sid=hf_synthetic_sid_9" },
                    { "name": "User-Agent", "value": "TestAgent/1.0" }
                  ],
                  "postData": { "mimeType": "application/json", "text": "{\"selectedItem\":\"item_1\"}" }
                },
                "response": { "status": 200, "content": { "mimeType": "application/json", "text": "{\"ok\":true}" } }
              }
            ]
          }
        }"#;

        let har = parse_har_bytes(raw, &ImportLimits::default()).expect("parse");
        let session = normalize_har_session("sess-0001", "all8.har", &har);
        let endpoints = discover_endpoints(std::slice::from_ref(&session));
        let extracted = extract_candidates(std::slice::from_ref(&session), &endpoints);
        assert_eq!(extracted.len(), 1);

        let e0 = &extracted[0].1[0];
        let e1 = &extracted[0].1[1];

        // Producers: SetCookie, ResponseHeader (incl. Location segment, excluding Date), ResponseBody
        assert!(e0
            .producers
            .iter()
            .any(|p| p.location == ValueLocation::SetCookie && p.path_or_key == "sid"));
        assert!(e0
            .producers
            .iter()
            .any(|p| p.location == ValueLocation::ResponseHeader
                && p.path_or_key == "Location"
                && p.derived_prefix_suffix
                && p.value == "ord_777"));
        assert!(!e0
            .producers
            .iter()
            .any(|p| p.path_or_key.eq_ignore_ascii_case("date")));
        assert!(e0
            .producers
            .iter()
            .any(|p| p.location == ValueLocation::ResponseBody
                && p.path_or_key == "$.items[0].id"
                && p.normalized_key == "$.items[*].id"));

        // Consumers: Path, Query, Header (excluding User-Agent), Cookie, Body (form + json)
        assert!(e1
            .consumers
            .iter()
            .any(|c| c.location == ValueLocation::Path && c.value == "ord_777"));
        assert!(e1
            .consumers
            .iter()
            .any(|c| c.location == ValueLocation::Query && c.path_or_key == "trace"));
        assert!(e1
            .consumers
            .iter()
            .any(|c| c.location == ValueLocation::Header && c.path_or_key == "Authorization"));
        assert!(!e1
            .consumers
            .iter()
            .any(|c| c.path_or_key.eq_ignore_ascii_case("user-agent")));
        assert!(e1
            .consumers
            .iter()
            .any(|c| c.location == ValueLocation::Cookie && c.path_or_key == "sid"));
        assert!(e1
            .consumers
            .iter()
            .any(|c| c.location == ValueLocation::Body && c.path_or_key == "$.selectedItem"));
        assert!(e0
            .consumers
            .iter()
            .any(|c| c.location == ValueLocation::Body && c.path_or_key == "grant_type"));
    }
}
