use std::collections::{BTreeMap, BTreeSet};

use harflow_classifier::classify_entry;
use harflow_model::{
    Endpoint, EndpointParameter, PaginationInfo, PaginationStyle, ParameterLocation,
};
use harflow_normalizer::{NormalizedEntry, NormalizedSession};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegmentParamKind {
    Numeric,
    Uuid,
    PrefixedId,
    SiblingSlug,
}

impl SegmentParamKind {
    const fn type_str(self) -> &'static str {
        match self {
            Self::Numeric => "integer",
            Self::Uuid => "uuid",
            Self::PrefixedId | Self::SiblingSlug => "string",
        }
    }
}

/// Groups normalized requests across sessions into logical API endpoints while preserving static routes.
#[must_use]
pub fn discover_endpoints(sessions: &[NormalizedSession]) -> Vec<Endpoint> {
    discover_endpoints_with_overrides(sessions, &BTreeMap::new())
}

/// Groups normalized requests while honoring manual `METHOD /path` or `METHOD host /path` template overrides.
#[must_use]
pub fn discover_endpoints_with_overrides(
    sessions: &[NormalizedSession],
    overrides: &BTreeMap<String, String>,
) -> Vec<Endpoint> {
    let entries: Vec<&NormalizedEntry> = sessions.iter().flat_map(|s| s.entries.iter()).collect();

    // Index segment cardinality by (method, host, seg_count, position) for conservative sibling inference
    // and to guard small static integers such as `/api/2/users`.
    let mut sibling_values: BTreeMap<(String, String, usize, usize), BTreeSet<String>> =
        BTreeMap::new();
    let mut path_segments_cache: Vec<Vec<String>> = Vec::with_capacity(entries.len());

    for entry in &entries {
        let segs: Vec<String> = entry
            .path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .collect();
        let seg_count = segs.len();
        for (pos, seg) in segs.iter().enumerate() {
            sibling_values
                .entry((entry.method.clone(), entry.host.clone(), seg_count, pos))
                .or_default()
                .insert(seg.clone());
        }
        path_segments_cache.push(segs);
    }

    // Track which interior segments act as varying slugs between identical static neighbors.
    let mut sibling_slug_positions: BTreeSet<(String, String, usize, usize)> = BTreeSet::new();
    let mut entries_by_shape: BTreeMap<(String, String, usize), Vec<&[String]>> = BTreeMap::new();
    for (entry, segs) in entries.iter().zip(path_segments_cache.iter()) {
        entries_by_shape
            .entry((entry.method.clone(), entry.host.clone(), segs.len()))
            .or_default()
            .push(segs.as_slice());
    }

    for ((method, host, seg_count), group_paths) in &entries_by_shape {
        if *seg_count < 3 || group_paths.len() < 3 {
            continue;
        }
        for pos in 1..(seg_count - 1) {
            let mut context_groups: BTreeMap<(&str, &str), BTreeSet<&str>> = BTreeMap::new();
            for segs in group_paths {
                let left = segs[pos - 1].as_str();
                let right = segs[pos + 1].as_str();
                context_groups
                    .entry((left, right))
                    .or_default()
                    .insert(segs[pos].as_str());
            }

            if context_groups.values().any(|vals| {
                vals.len() >= 3
                    && vals
                        .iter()
                        .all(|v| !is_version_segment(v) && !is_static_keyword(v))
            }) {
                sibling_slug_positions.insert((method.clone(), host.clone(), *seg_count, pos));
            }
        }
    }

    struct Group {
        method: String,
        host: String,
        path_template: String,
        category: String,
        status_codes: BTreeSet<u16>,
        status_distribution: BTreeMap<String, usize>,
        request_ids: Vec<String>,
        query_counts: BTreeMap<String, usize>,
        query_values: BTreeMap<String, Vec<String>>,
        path_params: Vec<(String, SegmentParamKind)>,
        distinct_path_tuples: BTreeSet<Vec<String>>,
        pagination_query_params: BTreeSet<String>,
        pagination_style: Option<PaginationStyle>,
        pagination_next_field: Option<String>,
    }

    let mut groups: BTreeMap<(String, String, String), Group> = BTreeMap::new();

    for (entry, segs) in entries.iter().zip(path_segments_cache.iter()) {
        let class = classify_entry(entry);
        let override_exact =
            overrides.get(&format!("{} {} {}", entry.method, entry.host, entry.path));
        let override_method_path = overrides.get(&format!("{} {}", entry.method, entry.path));

        let (template, path_params, captured_raw_params) =
            if let Some(custom_template) = override_exact.or(override_method_path) {
                let params = extract_template_params(custom_template);
                (custom_template.clone(), params, Vec::new())
            } else {
                parameterize_segments(entry, segs, &sibling_values, &sibling_slug_positions)
            };

        let key = (entry.method.clone(), entry.host.clone(), template.clone());
        let group = groups.entry(key).or_insert_with(|| Group {
            method: entry.method.clone(),
            host: entry.host.clone(),
            path_template: template,
            category: class.category.as_str().to_string(),
            status_codes: BTreeSet::new(),
            status_distribution: BTreeMap::new(),
            request_ids: Vec::new(),
            query_counts: BTreeMap::new(),
            query_values: BTreeMap::new(),
            path_params,
            distinct_path_tuples: BTreeSet::new(),
            pagination_query_params: BTreeSet::new(),
            pagination_style: None,
            pagination_next_field: None,
        });

        if !captured_raw_params.is_empty() {
            group.distinct_path_tuples.insert(captured_raw_params);
        }
        group.status_codes.insert(entry.response_status);
        *group
            .status_distribution
            .entry(entry.response_status.to_string())
            .or_insert(0) += 1;
        group.request_ids.push(entry.id.clone());

        for (qname, qval) in &entry.query_params {
            *group.query_counts.entry(qname.clone()).or_insert(0) += 1;
            if !qval.is_empty() {
                group
                    .query_values
                    .entry(qname.clone())
                    .or_default()
                    .push(qval.clone());
            }
            if let Some(style) = classify_pagination_query_key(qname) {
                group.pagination_query_params.insert(qname.clone());
                if group.pagination_style.is_none() {
                    group.pagination_style = Some(style);
                }
            }
        }

        if let Some(next_field) = detect_response_next_field(entry) {
            if group.pagination_next_field.is_none() {
                group.pagination_next_field = Some(next_field);
            }
            if group.pagination_style.is_none() {
                group.pagination_style = Some(PaginationStyle::Cursor);
            }
        }
    }

    groups
        .into_values()
        .enumerate()
        .map(|(idx, group)| {
            let total_obs = group.request_ids.len();
            let mut parameters: Vec<EndpointParameter> = group
                .path_params
                .iter()
                .map(|(name, kind)| EndpointParameter {
                    name: name.clone(),
                    location: ParameterLocation::Path,
                    inferred_type: kind.type_str().to_string(),
                    required: true,
                    observed_count: total_obs,
                })
                .collect();

            for (qname, count) in &group.query_counts {
                let vals = group
                    .query_values
                    .get(qname)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                parameters.push(EndpointParameter {
                    name: qname.clone(),
                    location: ParameterLocation::Query,
                    inferred_type: infer_query_param_type(vals).to_string(),
                    required: *count == total_obs && total_obs > 1,
                    observed_count: *count,
                });
            }

            let confidence = compute_endpoint_confidence(
                &group.path_params,
                total_obs,
                group.distinct_path_tuples.len(),
            );
            let pagination = group.pagination_style.map(|style| PaginationInfo {
                style,
                query_params: group.pagination_query_params.into_iter().collect(),
                response_next_field: group.pagination_next_field,
            });
            let operation_id = build_operation_id(&group.method, &group.path_template);

            Endpoint {
                id: format!("ep-{:04}", idx + 1),
                operation_id: Some(operation_id),
                method: group.method,
                host: group.host,
                path_template: group.path_template,
                parameters,
                request_schema_id: None,
                response_schema_id: None,
                status_codes: group.status_codes.into_iter().collect(),
                status_distribution: group.status_distribution,
                pagination,
                confidence,
                category: group.category,
                observed_request_ids: group.request_ids,
            }
        })
        .collect()
}

fn parameterize_segments(
    entry: &NormalizedEntry,
    segs: &[String],
    sibling_values: &BTreeMap<(String, String, usize, usize), BTreeSet<String>>,
    sibling_slug_positions: &BTreeSet<(String, String, usize, usize)>,
) -> (String, Vec<(String, SegmentParamKind)>, Vec<String>) {
    let mut param_names: Vec<(String, SegmentParamKind)> = Vec::new();
    let mut used_names: BTreeSet<String> = BTreeSet::new();
    let mut out_segments = Vec::with_capacity(segs.len());
    let mut raw_captured = Vec::new();
    let seg_count = segs.len();

    for (pos, segment) in segs.iter().enumerate() {
        let prev = if pos > 0 {
            Some(segs[pos - 1].as_str())
        } else {
            None
        };
        let sibling_cardinality = sibling_values
            .get(&(entry.method.clone(), entry.host.clone(), seg_count, pos))
            .map_or(1, BTreeSet::len);
        let is_sibling_slug = sibling_slug_positions.contains(&(
            entry.method.clone(),
            entry.host.clone(),
            seg_count,
            pos,
        ));

        let maybe_kind =
            classify_segment_param(segment, prev, sibling_cardinality, is_sibling_slug);
        if let Some(kind) = maybe_kind {
            let base_name = singularize_param_name(prev);
            let mut param_name = base_name.clone();
            let mut suffix = 2usize;
            while used_names.contains(&param_name) {
                param_name = format!("{base_name}_{suffix}");
                suffix += 1;
            }
            used_names.insert(param_name.clone());
            param_names.push((param_name.clone(), kind));
            raw_captured.push(segment.clone());
            out_segments.push(format!("{{{param_name}}}"));
        } else {
            out_segments.push(segment.clone());
        }
    }

    let template = if out_segments.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", out_segments.join("/"))
    };

    (template, param_names, raw_captured)
}

fn classify_segment_param(
    segment: &str,
    prev: Option<&str>,
    sibling_cardinality: usize,
    is_sibling_slug: bool,
) -> Option<SegmentParamKind> {
    if is_version_segment(segment) || is_static_keyword(segment) {
        return None;
    }

    if is_uuid_like(segment) {
        return Some(SegmentParamKind::Uuid);
    }

    if !segment.is_empty() && segment.chars().all(|c| c.is_ascii_digit()) {
        // Conservative guard: small single-digit API version segment right after `/api/` with a single observation stays static.
        if segment.len() == 1
            && prev.is_some_and(|p| p.eq_ignore_ascii_case("api"))
            && sibling_cardinality <= 1
        {
            return None;
        }
        return Some(SegmentParamKind::Numeric);
    }

    if is_prefixed_entity_id(segment) {
        return Some(SegmentParamKind::PrefixedId);
    }

    if is_sibling_slug {
        return Some(SegmentParamKind::SiblingSlug);
    }

    None
}

fn is_version_segment(segment: &str) -> bool {
    let lower = segment.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix('v') {
        if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
            return true;
        }
    }
    matches!(
        lower.as_str(),
        "oauth2" | "web3" | "2fa" | "mfa" | "3ds" | "s3" | "base64" | "sha256"
    )
}

fn is_static_keyword(segment: &str) -> bool {
    matches!(
        segment.to_ascii_lowercase().as_str(),
        "api"
            | "me"
            | "self"
            | "list"
            | "create"
            | "update"
            | "delete"
            | "search"
            | "export"
            | "status"
            | "health"
            | "metrics"
            | "settings"
            | "profile"
            | "summary"
    )
}

fn is_prefixed_entity_id(segment: &str) -> bool {
    let Some((prefix, rest)) = segment.split_once('_') else {
        return false;
    };
    (2..=6).contains(&prefix.len())
        && prefix.chars().all(|c| c.is_ascii_alphabetic())
        && !rest.is_empty()
        && rest.chars().any(|c| c.is_ascii_digit())
        && rest
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn is_uuid_like(segment: &str) -> bool {
    let parts: Vec<&str> = segment.split('-').collect();
    parts.len() == 5
        && parts[0].len() == 8
        && parts[1].len() == 4
        && parts[2].len() == 4
        && parts[3].len() == 4
        && parts[4].len() == 12
        && parts
            .iter()
            .all(|p| p.chars().all(|c| c.is_ascii_hexdigit()))
}

fn singularize_param_name(prev_segment: Option<&str>) -> String {
    let Some(raw) = prev_segment else {
        return "id".to_string();
    };
    let clean: String = raw
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let clean = clean.trim_matches('_');

    if clean.is_empty() || is_version_segment(clean) || clean == "api" {
        return "id".to_string();
    }

    let singular = if let Some(stem) = clean.strip_suffix("ies") {
        if stem.is_empty() {
            clean.to_string()
        } else {
            format!("{stem}y")
        }
    } else if clean.ends_with("sses") || clean.ends_with("xes") || clean.ends_with("ches") {
        clean.trim_end_matches("es").to_string()
    } else if clean.len() > 2 && clean.ends_with('s') && !clean.ends_with("ss") {
        clean[..clean.len() - 1].to_string()
    } else {
        clean.to_string()
    };

    format!("{singular}_id")
}

fn extract_template_params(template: &str) -> Vec<(String, SegmentParamKind)> {
    template
        .split('/')
        .filter(|s| s.starts_with('{') && s.ends_with('}') && s.len() > 2)
        .map(|s| (s[1..s.len() - 1].to_string(), SegmentParamKind::SiblingSlug))
        .collect()
}

fn infer_query_param_type(values: &[String]) -> &'static str {
    if values.is_empty() {
        return "string";
    }

    let mut seen = BTreeSet::new();
    for v in values {
        let trimmed = v.trim();
        let kind = if trimmed.eq_ignore_ascii_case("true") || trimmed.eq_ignore_ascii_case("false")
        {
            "boolean"
        } else if trimmed.parse::<i64>().is_ok() {
            "integer"
        } else if trimmed.parse::<f64>().is_ok() {
            "number"
        } else if is_uuid_like(trimmed) {
            "uuid"
        } else if is_iso_datetime(trimmed) {
            "datetime"
        } else {
            "string"
        };
        seen.insert(kind);
    }

    if seen.len() == 1 {
        seen.into_iter().next().unwrap_or("string")
    } else if seen.len() == 2 && seen.contains("integer") && seen.contains("number") {
        "number"
    } else {
        "string"
    }
}

fn is_iso_datetime(s: &str) -> bool {
    s.len() >= 20
        && s.as_bytes().get(4) == Some(&b'-')
        && s.as_bytes().get(7) == Some(&b'-')
        && (s.contains('T') || s.contains('t'))
        && (s.ends_with('Z') || s.contains('+'))
}

fn classify_pagination_query_key(name: &str) -> Option<PaginationStyle> {
    match name.trim().to_ascii_lowercase().as_str() {
        "cursor" | "after" | "before" | "page_token" | "next_page_token" => {
            Some(PaginationStyle::Cursor)
        }
        "page" | "per_page" | "page_size" => Some(PaginationStyle::PageNumber),
        "offset" | "limit" => Some(PaginationStyle::OffsetLimit),
        _ => None,
    }
}

fn detect_response_next_field(entry: &NormalizedEntry) -> Option<String> {
    let obj = entry.response_body_json.as_ref()?.as_object()?;
    for candidate in [
        "next_cursor",
        "nextCursor",
        "next_page_token",
        "nextPageToken",
        "has_more",
        "hasMore",
    ] {
        if obj.contains_key(candidate) {
            return Some(candidate.to_string());
        }
    }
    None
}

fn compute_endpoint_confidence(
    path_params: &[(String, SegmentParamKind)],
    total_obs: usize,
    distinct_path_tuples: usize,
) -> f64 {
    if path_params.is_empty() {
        return if total_obs > 1 { 0.95 } else { 0.85 };
    }
    if path_params
        .iter()
        .any(|(_, k)| matches!(k, SegmentParamKind::SiblingSlug))
    {
        return 0.80;
    }
    if path_params
        .iter()
        .any(|(_, k)| matches!(k, SegmentParamKind::Uuid))
    {
        return 0.96;
    }
    if distinct_path_tuples > 1 || total_obs > 1 {
        0.95
    } else {
        0.85
    }
}

fn build_operation_id(method: &str, path_template: &str) -> String {
    let mut parts = vec![method.to_ascii_lowercase()];
    for seg in path_template.split('/').filter(|s| !s.is_empty()) {
        let cleaned = seg
            .trim_start_matches('{')
            .trim_end_matches('}')
            .replace('-', "_");
        if !cleaned.is_empty() {
            parts.push(cleaned.to_ascii_lowercase());
        }
    }
    parts.join("_")
}

#[cfg(test)]
mod tests {
    use harflow_har::{parse_har_bytes, ImportLimits};
    use harflow_normalizer::normalize_har_session;

    use super::*;

    #[test]
    fn groups_numeric_ids_with_singular_names_and_preserves_version_prefix() {
        let raw = br#"{
          "log": {
            "version": "1.2",
            "entries": [
              {
                "request": { "method": "GET", "url": "https://api.example.test/v1/users/101" },
                "response": { "status": 200, "content": { "mimeType": "application/json" } }
              },
              {
                "request": { "method": "GET", "url": "https://api.example.test/v1/users/202" },
                "response": { "status": 200, "content": { "mimeType": "application/json" } }
              }
            ]
          }
        }"#;
        let har = parse_har_bytes(raw, &ImportLimits::default()).expect("parse");
        let session = normalize_har_session("s1", "demo.har", &har);
        let endpoints = discover_endpoints(&[session]);

        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].path_template, "/v1/users/{user_id}");
        assert_eq!(endpoints[0].parameters[0].inferred_type, "integer");
        assert_eq!(endpoints[0].observed_request_ids.len(), 2);
        assert_eq!(endpoints[0].status_distribution.get("200"), Some(&2));
    }

    #[test]
    fn preserves_static_small_digit_version_after_api_and_detects_prefixed_ids() {
        let raw = br#"{
          "log": {
            "version": "1.2",
            "entries": [
              {
                "request": { "method": "GET", "url": "https://api.example.test/api/2/users" },
                "response": { "status": 200, "content": { "mimeType": "application/json" } }
              },
              {
                "request": { "method": "GET", "url": "https://api.example.test/v1/orders/ord_synthetic_9001" },
                "response": { "status": 200, "content": { "mimeType": "application/json" } }
              },
              {
                "request": { "method": "GET", "url": "https://api.example.test/v1/orders/ord_synthetic_9002" },
                "response": { "status": 404, "content": { "mimeType": "application/json" } }
              }
            ]
          }
        }"#;
        let har = parse_har_bytes(raw, &ImportLimits::default()).expect("parse");
        let session = normalize_har_session("s1", "demo.har", &har);
        let endpoints = discover_endpoints(&[session]);

        let static_ep = endpoints
            .iter()
            .find(|e| e.path_template == "/api/2/users")
            .expect("static /api/2/users preserved");
        assert!(static_ep.parameters.is_empty());

        let orders_ep = endpoints
            .iter()
            .find(|e| e.path_template == "/v1/orders/{order_id}")
            .expect("prefixed order IDs grouped");
        assert_eq!(orders_ep.status_codes, vec![200, 404]);
        assert_eq!(orders_ep.status_distribution.get("404"), Some(&1));
    }

    #[test]
    fn detects_sibling_slugs_optional_typed_query_params_and_cursor_pagination() {
        let raw = br#"{
          "log": {
            "version": "1.2",
            "entries": [
              {
                "request": { "method": "GET", "url": "https://api.example.test/v1/orgs/acme/members?limit=20&cursor=cur_1&active=true" },
                "response": { "status": 200, "content": { "mimeType": "application/json", "text": "{\"items\":[],\"nextCursor\":\"cur_2\"}" } }
              },
              {
                "request": { "method": "GET", "url": "https://api.example.test/v1/orgs/globex/members?limit=20" },
                "response": { "status": 200, "content": { "mimeType": "application/json", "text": "{\"items\":[]}" } }
              },
              {
                "request": { "method": "GET", "url": "https://api.example.test/v1/orgs/initech/members?limit=50" },
                "response": { "status": 200, "content": { "mimeType": "application/json", "text": "{\"items\":[]}" } }
              }
            ]
          }
        }"#;
        let har = parse_har_bytes(raw, &ImportLimits::default()).expect("parse");
        let session = normalize_har_session("s1", "orgs.har", &har);
        let endpoints = discover_endpoints(&[session]);

        assert_eq!(endpoints.len(), 1);
        let ep = &endpoints[0];
        assert_eq!(ep.path_template, "/v1/orgs/{org_id}/members");
        assert!((ep.confidence - 0.80).abs() < 1e-6);

        let limit_p = ep
            .parameters
            .iter()
            .find(|p| p.name == "limit")
            .expect("limit param");
        assert_eq!(limit_p.inferred_type, "integer");
        assert!(limit_p.required);

        let active_p = ep
            .parameters
            .iter()
            .find(|p| p.name == "active")
            .expect("active param");
        assert_eq!(active_p.inferred_type, "boolean");
        assert!(!active_p.required);

        let pagination = ep.pagination.as_ref().expect("pagination detected");
        assert_eq!(pagination.style, PaginationStyle::Cursor);
        assert_eq!(
            pagination.response_next_field.as_deref(),
            Some("nextCursor")
        );
    }
}
