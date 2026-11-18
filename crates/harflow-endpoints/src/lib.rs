use std::collections::{BTreeMap, BTreeSet};

use harflow_classifier::classify_entry;
use harflow_model::{Endpoint, EndpointParameter, ParameterLocation};
use harflow_normalizer::NormalizedSession;

/// Groups normalized requests across sessions into logical API endpoints while preserving static segments such as `v1`.
#[must_use]
pub fn discover_endpoints(sessions: &[NormalizedSession]) -> Vec<Endpoint> {
    struct Group {
        method: String,
        host: String,
        path_template: String,
        category: String,
        status_codes: BTreeSet<u16>,
        request_ids: Vec<String>,
        query_counts: BTreeMap<String, usize>,
        param_names: Vec<String>,
    }

    let mut groups: BTreeMap<(String, String, String), Group> = BTreeMap::new();

    for session in sessions {
        for entry in &session.entries {
            let class = classify_entry(entry);
            let (template, path_params) = parameterize_path(&entry.path);
            let key = (entry.method.clone(), entry.host.clone(), template.clone());

            let group = groups.entry(key).or_insert_with(|| Group {
                method: entry.method.clone(),
                host: entry.host.clone(),
                path_template: template,
                category: class.category.as_str().to_string(),
                status_codes: BTreeSet::new(),
                request_ids: Vec::new(),
                query_counts: BTreeMap::new(),
                param_names: path_params,
            });

            group.status_codes.insert(entry.response_status);
            group.request_ids.push(entry.id.clone());
            for (qname, _) in &entry.query_params {
                *group.query_counts.entry(qname.clone()).or_insert(0) += 1;
            }
        }
    }

    groups
        .into_values()
        .enumerate()
        .map(|(idx, group)| {
            let total_obs = group.request_ids.len();
            let mut parameters: Vec<EndpointParameter> = group
                .param_names
                .into_iter()
                .map(|name| EndpointParameter {
                    name,
                    location: ParameterLocation::Path,
                    inferred_type: "string".to_string(),
                    required: true,
                    observed_count: total_obs,
                })
                .collect();

            for (qname, count) in group.query_counts {
                parameters.push(EndpointParameter {
                    name: qname,
                    location: ParameterLocation::Query,
                    inferred_type: "string".to_string(),
                    required: count == total_obs && total_obs > 1,
                    observed_count: count,
                });
            }

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
                confidence: if total_obs > 1 { 0.95 } else { 0.80 },
                category: group.category,
                observed_request_ids: group.request_ids,
            }
        })
        .collect()
}

fn parameterize_path(path: &str) -> (String, Vec<String>) {
    let mut param_names = Vec::new();
    let mut out_segments = Vec::new();

    for segment in path.split('/').filter(|s| !s.is_empty()) {
        if looks_like_id_segment(segment) {
            let param_name = if param_names.is_empty() {
                "id".to_string()
            } else {
                format!("id_{}", param_names.len() + 1)
            };
            param_names.push(param_name.clone());
            out_segments.push(format!("{{{param_name}}}"));
        } else {
            out_segments.push(segment.to_string());
        }
    }

    if out_segments.is_empty() {
        ("/".to_string(), param_names)
    } else {
        (format!("/{}", out_segments.join("/")), param_names)
    }
}

fn looks_like_id_segment(segment: &str) -> bool {
    if is_version_segment(segment) {
        return false;
    }
    if segment.chars().all(|c| c.is_ascii_digit()) && !segment.is_empty() {
        return true;
    }
    is_uuid_like(segment)
}

fn is_version_segment(segment: &str) -> bool {
    let lower = segment.to_ascii_lowercase();
    lower.starts_with('v') && lower.len() >= 2 && lower[1..].chars().all(|c| c.is_ascii_digit())
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
    fn groups_numeric_ids_while_preserving_version_prefix() {
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
        assert_eq!(endpoints[0].path_template, "/v1/users/{id}");
        assert_eq!(endpoints[0].observed_request_ids.len(), 2);
    }
}
