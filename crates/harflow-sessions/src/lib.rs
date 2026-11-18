use std::collections::{BTreeMap, BTreeSet, HashMap};

use harflow_endpoints::discover_endpoints;
use harflow_model::{
    AuthFlow, AuthScheme, Dependency, DependencyType, Endpoint, ParameterLocation, SchemaDefinition,
};
use harflow_normalizer::{NormalizedEntry, NormalizedSession};
use harflow_schema::{diff_schemas, infer_endpoint_schemas, SchemaDiff};
use harflow_security::parse_redacted_marker;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Summary of observed authentication signals without retaining raw credential values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSchemeSummary {
    pub scheme: AuthScheme,
    pub observed_count: usize,
}

/// Redacted token or cookie rotation transition observed inside a single session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenRotation {
    pub session_id: String,
    pub kind: String,
    pub from_request_id: String,
    pub from_fingerprint: String,
    pub to_request_id: String,
    pub to_fingerprint: String,
}

/// Expiration or unauthorized response indicator observed during a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpirationSignal {
    pub session_id: String,
    pub request_id: String,
    pub kind: String,
    pub detail: String,
}

/// Consolidated session lifecycle & authentication intelligence report (SRS M08).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionLifecycleReport {
    pub auth_flows: Vec<AuthFlow>,
    pub login_request_ids: Vec<String>,
    pub refresh_request_ids: Vec<String>,
    pub token_rotations: Vec<TokenRotation>,
    pub expiration_signals: Vec<ExpirationSignal>,
    pub auto_refresh_eligible: bool,
}

/// Parameter stability classification across sessions and requests (SRS M09).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariabilityKind {
    Constant,
    SessionScoped,
    RequestVolatile,
}

/// Parameter variability breakdown for one endpoint input across analyzed sessions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterVariability {
    pub endpoint_key: String,
    pub location: ParameterLocation,
    pub name: String,
    pub variability: VariabilityKind,
    pub distinct_values_count: usize,
    pub sample_values: Vec<String>,
}

/// Endpoint observed exclusively in a single session when comparing multiple HAR captures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEndpointPresence {
    pub session_id: String,
    pub endpoint_key: String,
}

/// Difference in observed authentication scheme usage across sessions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSessionDiff {
    pub scheme: AuthScheme,
    pub present_in_sessions: Vec<String>,
    pub missing_in_sessions: Vec<String>,
}

/// Inferred request/response schema drift for one endpoint between two sessions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaDriftEntry {
    pub endpoint_key: String,
    pub baseline_session_id: String,
    pub compared_session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_diff: Option<SchemaDiff>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_diff: Option<SchemaDiff>,
}

/// Multi-HAR session comparison report (SRS M09).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionComparisonReport {
    pub session_ids: Vec<String>,
    pub shared_endpoints: Vec<String>,
    pub session_only_endpoints: Vec<SessionEndpointPresence>,
    pub parameter_variability: Vec<ParameterVariability>,
    pub corroborated_dependency_ids: Vec<String>,
    pub single_session_dependency_ids: Vec<String>,
    pub auth_differences: Vec<AuthSessionDiff>,
    pub schema_drifts: Vec<SchemaDriftEntry>,
}

/// Detects observed Bearer, Cookie, CSRF, API-key, OAuth-like, and signature schemes across sessions
/// without retaining raw credentials.
#[must_use]
pub fn detect_observed_auth(sessions: &[NormalizedSession]) -> Vec<AuthFlow> {
    let mut bearer_ids = Vec::new();
    let mut cookie_ids = Vec::new();
    let mut csrf_ids = Vec::new();
    let mut api_key_ids = Vec::new();
    let mut oauth_ids = Vec::new();
    let mut signature_ids = Vec::new();

    for session in sessions {
        for entry in &session.entries {
            if entry_has_bearer(entry) {
                bearer_ids.push(entry.id.clone());
            }
            if entry_has_cookie(entry) {
                cookie_ids.push(entry.id.clone());
            }
            if entry_has_csrf(entry) {
                csrf_ids.push(entry.id.clone());
            }
            if entry_has_api_key(entry) {
                api_key_ids.push(entry.id.clone());
            }
            if entry_is_oauth_like(entry) {
                oauth_ids.push(entry.id.clone());
            }
            if entry_has_unknown_signature(entry) {
                signature_ids.push(entry.id.clone());
            }
        }
    }

    let mut flows = Vec::new();
    if !bearer_ids.is_empty() {
        flows.push(AuthFlow {
            id: "auth-bearer".to_string(),
            scheme: AuthScheme::Bearer,
            description: "Observed Authorization Bearer header (redacted before storage)"
                .to_string(),
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
            description: "Observed CSRF header or form token usage".to_string(),
            request_ids: csrf_ids,
            confidence: 0.85,
        });
    }
    if !api_key_ids.is_empty() {
        flows.push(AuthFlow {
            id: "auth-api-key".to_string(),
            scheme: AuthScheme::ApiKey,
            description: "Observed API key header or query parameter (redacted before storage)"
                .to_string(),
            request_ids: api_key_ids,
            confidence: 0.88,
        });
    }
    if !oauth_ids.is_empty() {
        flows.push(AuthFlow {
            id: "auth-oauth-like".to_string(),
            scheme: AuthScheme::OauthLike,
            description:
                "Observed OAuth2/token endpoint exchange (grant_type / access_token / refresh_token)"
                    .to_string(),
            request_ids: oauth_ids,
            confidence: 0.90,
        });
    }
    if !signature_ids.is_empty() {
        flows.push(AuthFlow {
            id: "auth-unknown-signature".to_string(),
            scheme: AuthScheme::Unknown,
            description: "Observed custom request signature header requiring manual extension hook"
                .to_string(),
            request_ids: signature_ids,
            confidence: 0.50,
        });
    }

    flows
}

/// Analyzes login/refresh endpoints, redacted token rotations, expiration signals, and whether
/// deterministic auto-refresh generation is strictly justified by observed proof.
#[must_use]
pub fn analyze_session_lifecycle(
    sessions: &[NormalizedSession],
    dependencies: &[Dependency],
) -> SessionLifecycleReport {
    let auth_flows = detect_observed_auth(sessions);
    let mut login_request_ids = Vec::new();
    let mut refresh_request_ids = Vec::new();
    let mut token_rotations = Vec::new();
    let mut expiration_signals = Vec::new();

    for session in sessions {
        let mut last_fp_by_kind: HashMap<String, (String, String)> = HashMap::new();

        for entry in &session.entries {
            if is_refresh_request(entry) {
                refresh_request_ids.push(entry.id.clone());
            } else if is_login_request(entry) {
                login_request_ids.push(entry.id.clone());
            }

            if matches!(entry.response_status, 401 | 403) {
                expiration_signals.push(ExpirationSignal {
                    session_id: session.session_id.clone(),
                    request_id: entry.id.clone(),
                    kind: format!("http_{}", entry.response_status),
                    detail: format!(
                        "HTTP {} authorization/session challenge",
                        entry.response_status
                    ),
                });
            }

            if let Some(expires_in) = extract_expires_in(entry.response_body_json.as_ref()) {
                expiration_signals.push(ExpirationSignal {
                    session_id: session.session_id.clone(),
                    request_id: entry.id.clone(),
                    kind: "expires_in".to_string(),
                    detail: format!("expires_in={expires_in}"),
                });
            }

            // Track redacted credential/cookie fingerprints per kind for rotation detection.
            for (name, val) in &entry.request_headers {
                if name.eq_ignore_ascii_case("cookie") {
                    continue;
                }
                if let Some((kind, fp)) = parse_redacted_marker(val) {
                    record_fingerprint_transition(
                        &session.session_id,
                        &entry.id,
                        kind,
                        fp,
                        &mut last_fp_by_kind,
                        &mut token_rotations,
                    );
                }
            }
            for (cookie_name, val) in &entry.response_cookies {
                if let Some((_, fp)) = parse_redacted_marker(val) {
                    let key = format!("cookie:{cookie_name}");
                    record_fingerprint_transition(
                        &session.session_id,
                        &entry.id,
                        &key,
                        fp,
                        &mut last_fp_by_kind,
                        &mut token_rotations,
                    );
                }
            }
            if let Some(json) = &entry.response_body_json {
                for field in [
                    "access_token",
                    "accessToken",
                    "refresh_token",
                    "refreshToken",
                ] {
                    if let Some(val) = json.get(field).and_then(Value::as_str) {
                        if let Some((kind, fp)) = parse_redacted_marker(val) {
                            let canonical_kind = if field.to_ascii_lowercase().contains("refresh") {
                                "refresh_token"
                            } else if field.to_ascii_lowercase().contains("access") {
                                "access_token"
                            } else {
                                kind
                            };
                            record_fingerprint_transition(
                                &session.session_id,
                                &entry.id,
                                canonical_kind,
                                fp,
                                &mut last_fp_by_kind,
                                &mut token_rotations,
                            );
                        }
                    }
                }
            }
        }
    }

    let refresh_set: BTreeSet<&str> = refresh_request_ids.iter().map(String::as_str).collect();
    let has_refresh_chain_proof = dependencies.iter().any(|dep| {
        dep.dependency_type == DependencyType::SessionChain
            && (refresh_set.contains(dep.source.request_id.as_str())
                || refresh_set.contains(dep.target.request_id.as_str()))
    });

    let auto_refresh_eligible =
        !refresh_request_ids.is_empty() && (!token_rotations.is_empty() || has_refresh_chain_proof);

    SessionLifecycleReport {
        auth_flows,
        login_request_ids,
        refresh_request_ids,
        token_rotations,
        expiration_signals,
        auto_refresh_eligible,
    }
}

/// Compares multiple normalized HAR sessions cleanly without cross-contaminating secrets or tokens.
#[must_use]
pub fn compare_sessions(
    sessions: &[NormalizedSession],
    dependencies: &[Dependency],
) -> SessionComparisonReport {
    let session_ids: Vec<String> = sessions.iter().map(|s| s.session_id.clone()).collect();
    if sessions.is_empty() {
        return SessionComparisonReport::default();
    }

    let global_endpoints = discover_endpoints(sessions);
    let mut req_to_ep: HashMap<&str, &Endpoint> = HashMap::new();
    for ep in &global_endpoints {
        for req_id in &ep.observed_request_ids {
            req_to_ep.insert(req_id.as_str(), ep);
        }
    }

    let mut ep_sessions: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    // (endpoint_key, loc_tag, param_name) -> session_id -> BTreeSet<value>
    type ParamKey = (String, u8, String);
    let mut param_obs: BTreeMap<ParamKey, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();

    for session in sessions {
        for entry in &session.entries {
            let Some(ep) = req_to_ep.get(entry.id.as_str()) else {
                continue;
            };
            let ep_key = format!("{} {} {}", ep.method, ep.host, ep.path_template);
            ep_sessions
                .entry(ep_key.clone())
                .or_default()
                .insert(session.session_id.clone());

            // Collect path param observations.
            let tpl_segs: Vec<&str> = ep
                .path_template
                .split('/')
                .filter(|s| !s.is_empty())
                .collect();
            let raw_segs: Vec<&str> = entry.path.split('/').filter(|s| !s.is_empty()).collect();
            for (tpl, raw) in tpl_segs.iter().zip(raw_segs.iter()) {
                if let Some(name) = tpl.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                    param_obs
                        .entry((ep_key.clone(), 0, name.to_string()))
                        .or_default()
                        .entry(session.session_id.clone())
                        .or_default()
                        .insert((*raw).to_string());
                }
            }

            for (name, val) in &entry.query_params {
                param_obs
                    .entry((ep_key.clone(), 1, name.clone()))
                    .or_default()
                    .entry(session.session_id.clone())
                    .or_default()
                    .insert(val.clone());
            }

            for (name, val) in &entry.request_cookies {
                param_obs
                    .entry((ep_key.clone(), 3, name.clone()))
                    .or_default()
                    .entry(session.session_id.clone())
                    .or_default()
                    .insert(val.clone());
            }

            for (name, val) in &entry.request_headers {
                let lower = name.trim().to_ascii_lowercase();
                if matches!(
                    lower.as_str(),
                    "accept"
                        | "accept-encoding"
                        | "accept-language"
                        | "user-agent"
                        | "host"
                        | "origin"
                        | "referer"
                        | "connection"
                        | "content-type"
                        | "content-length"
                        | "cache-control"
                        | "pragma"
                        | "cookie"
                ) || lower.starts_with("sec-")
                {
                    continue;
                }
                param_obs
                    .entry((ep_key.clone(), 2, name.clone()))
                    .or_default()
                    .entry(session.session_id.clone())
                    .or_default()
                    .insert(val.clone());
            }
        }
    }

    let mut shared_endpoints = Vec::new();
    let mut session_only_endpoints = Vec::new();
    for (ep_key, sids) in &ep_sessions {
        if sids.len() >= 2 {
            shared_endpoints.push(ep_key.clone());
        } else if sessions.len() >= 2 {
            if let Some(only_sid) = sids.iter().next() {
                session_only_endpoints.push(SessionEndpointPresence {
                    session_id: only_sid.clone(),
                    endpoint_key: ep_key.clone(),
                });
            }
        }
    }

    let parameter_variability = param_obs
        .into_iter()
        .map(|((endpoint_key, loc_tag, name), by_session)| {
            let location = match loc_tag {
                0 => ParameterLocation::Path,
                1 => ParameterLocation::Query,
                2 => ParameterLocation::Header,
                _ => ParameterLocation::Cookie,
            };
            let mut global_values = BTreeSet::new();
            let mut all_sessions_single_valued = true;
            for vals in by_session.values() {
                if vals.len() > 1 {
                    all_sessions_single_valued = false;
                }
                for v in vals {
                    global_values.insert(v.clone());
                }
            }

            let distinct_values_count = global_values.len();
            let variability = if distinct_values_count <= 1 {
                VariabilityKind::Constant
            } else if all_sessions_single_valued && by_session.len() >= 2 {
                VariabilityKind::SessionScoped
            } else {
                VariabilityKind::RequestVolatile
            };

            let sample_values: Vec<String> = global_values.into_iter().take(5).collect();
            ParameterVariability {
                endpoint_key,
                location,
                name,
                variability,
                distinct_values_count,
                sample_values,
            }
        })
        .collect();

    let mut corroborated_dependency_ids = Vec::new();
    let mut single_session_dependency_ids = Vec::new();
    for dep in dependencies {
        if dep.observed_session_ids.len() >= 2 {
            corroborated_dependency_ids.push(dep.id.clone());
        } else {
            single_session_dependency_ids.push(dep.id.clone());
        }
    }

    let auth_differences = compute_auth_differences(sessions);
    let schema_drifts = compute_schema_drifts(sessions, &global_endpoints);

    SessionComparisonReport {
        session_ids,
        shared_endpoints,
        session_only_endpoints,
        parameter_variability,
        corroborated_dependency_ids,
        single_session_dependency_ids,
        auth_differences,
        schema_drifts,
    }
}

fn compute_auth_differences(sessions: &[NormalizedSession]) -> Vec<AuthSessionDiff> {
    if sessions.len() < 2 {
        return Vec::new();
    }

    let all_schemes = [
        AuthScheme::Bearer,
        AuthScheme::CookieSession,
        AuthScheme::Csrf,
        AuthScheme::ApiKey,
        AuthScheme::OauthLike,
        AuthScheme::Unknown,
    ];

    let per_session: Vec<(&str, Vec<AuthFlow>)> = sessions
        .iter()
        .map(|s| {
            (
                s.session_id.as_str(),
                detect_observed_auth(std::slice::from_ref(s)),
            )
        })
        .collect();

    let mut diffs = Vec::new();
    for scheme in all_schemes {
        let mut present = Vec::new();
        let mut missing = Vec::new();
        for (sid, flows) in &per_session {
            if flows.iter().any(|f| f.scheme == scheme) {
                present.push((*sid).to_string());
            } else {
                missing.push((*sid).to_string());
            }
        }
        if !present.is_empty() && !missing.is_empty() {
            diffs.push(AuthSessionDiff {
                scheme,
                present_in_sessions: present,
                missing_in_sessions: missing,
            });
        }
    }
    diffs
}

fn compute_schema_drifts(
    sessions: &[NormalizedSession],
    global_endpoints: &[Endpoint],
) -> Vec<SchemaDriftEntry> {
    if sessions.len() < 2 {
        return Vec::new();
    }

    type EndpointSchemasMap =
        BTreeMap<String, (Option<SchemaDefinition>, Option<SchemaDefinition>)>;
    let mut per_session_schemas: Vec<(&str, EndpointSchemasMap)> =
        Vec::with_capacity(sessions.len());

    for session in sessions {
        let entry_ids: BTreeSet<&str> = session.entries.iter().map(|e| e.id.as_str()).collect();
        let mut cloned_eps: Vec<Endpoint> = global_endpoints
            .iter()
            .filter_map(|ep| {
                let filtered_reqs: Vec<String> = ep
                    .observed_request_ids
                    .iter()
                    .filter(|id| entry_ids.contains(id.as_str()))
                    .cloned()
                    .collect();
                if filtered_reqs.is_empty() {
                    None
                } else {
                    let mut c = ep.clone();
                    c.observed_request_ids = filtered_reqs;
                    Some(c)
                }
            })
            .collect();

        let schemas = infer_endpoint_schemas(&mut cloned_eps, std::slice::from_ref(session));
        let schema_by_id: HashMap<&str, &SchemaDefinition> =
            schemas.iter().map(|s| (s.id.as_str(), s)).collect();

        let mut map = BTreeMap::new();
        for ep in &cloned_eps {
            let ep_key = format!("{} {} {}", ep.method, ep.host, ep.path_template);
            let req_schema = ep
                .request_schema_id
                .as_deref()
                .and_then(|id| schema_by_id.get(id).copied().cloned());
            let resp_schema = ep
                .response_schema_id
                .as_deref()
                .and_then(|id| schema_by_id.get(id).copied().cloned());
            map.insert(ep_key, (req_schema, resp_schema));
        }
        per_session_schemas.push((session.session_id.as_str(), map));
    }

    let mut drifts = Vec::new();
    let (base_sid, base_map) = &per_session_schemas[0];

    for (other_sid, other_map) in &per_session_schemas[1..] {
        for (ep_key, (base_req, base_resp)) in base_map {
            let Some((other_req, other_resp)) = other_map.get(ep_key) else {
                continue;
            };
            let request_diff = match (base_req, other_req) {
                (Some(a), Some(b)) => {
                    let d = diff_schemas(a, b);
                    (!is_empty_diff(&d)).then_some(d)
                }
                _ => None,
            };
            let response_diff = match (base_resp, other_resp) {
                (Some(a), Some(b)) => {
                    let d = diff_schemas(a, b);
                    (!is_empty_diff(&d)).then_some(d)
                }
                _ => None,
            };

            if request_diff.is_some() || response_diff.is_some() {
                drifts.push(SchemaDriftEntry {
                    endpoint_key: ep_key.clone(),
                    baseline_session_id: (*base_sid).to_string(),
                    compared_session_id: (*other_sid).to_string(),
                    request_diff,
                    response_diff,
                });
            }
        }
    }

    drifts
}

fn is_empty_diff(diff: &SchemaDiff) -> bool {
    diff.added_fields.is_empty() && diff.removed_fields.is_empty() && diff.changed_fields.is_empty()
}

fn record_fingerprint_transition(
    session_id: &str,
    request_id: &str,
    kind: &str,
    fp: &str,
    last_by_kind: &mut HashMap<String, (String, String)>,
    rotations: &mut Vec<TokenRotation>,
) {
    match last_by_kind.get(kind) {
        Some((prev_req, prev_fp)) if prev_fp != fp => {
            rotations.push(TokenRotation {
                session_id: session_id.to_string(),
                kind: kind.to_string(),
                from_request_id: prev_req.clone(),
                from_fingerprint: prev_fp.clone(),
                to_request_id: request_id.to_string(),
                to_fingerprint: fp.to_string(),
            });
            last_by_kind.insert(kind.to_string(), (request_id.to_string(), fp.to_string()));
        }
        None => {
            last_by_kind.insert(kind.to_string(), (request_id.to_string(), fp.to_string()));
        }
        _ => {}
    }
}

fn entry_has_bearer(entry: &NormalizedEntry) -> bool {
    entry.request_headers.iter().any(|(name, val)| {
        let lower_name = name.trim().to_ascii_lowercase();
        let lower_val = val.to_ascii_lowercase();
        matches!(lower_name.as_str(), "authorization" | "proxy-authorization")
            && (lower_val.contains("authorization_bearer")
                || lower_val.contains("bearer")
                || lower_val.contains("authorization"))
    })
}

fn entry_has_cookie(entry: &NormalizedEntry) -> bool {
    !entry.request_cookies.is_empty()
        || !entry.response_cookies.is_empty()
        || entry
            .request_headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("cookie"))
        || entry
            .response_headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("set-cookie"))
}

fn entry_has_csrf(entry: &NormalizedEntry) -> bool {
    entry.request_headers.iter().any(|(name, _)| {
        let lower = name.to_ascii_lowercase();
        lower == "x-csrf-token" || lower == "x-xsrf-token"
    }) || entry.request_form_params.iter().any(|(name, _)| {
        matches!(
            name.to_ascii_lowercase().as_str(),
            "csrf_token" | "xsrf_token"
        )
    })
}

fn entry_has_api_key(entry: &NormalizedEntry) -> bool {
    entry.request_headers.iter().any(|(name, _)| {
        let lower = name.trim().to_ascii_lowercase();
        matches!(lower.as_str(), "x-api-key" | "api-key" | "x-goog-api-key")
    }) || entry.query_params.iter().any(|(name, _)| {
        let lower = name.trim().to_ascii_lowercase();
        matches!(lower.as_str(), "api_key" | "apikey")
    })
}

fn entry_is_oauth_like(entry: &NormalizedEntry) -> bool {
    let lower_path = entry.path.to_ascii_lowercase();
    let token_path = lower_path.contains("/oauth/")
        || lower_path.ends_with("/token")
        || lower_path.contains("/auth/token")
        || lower_path.contains("/auth/login")
        || lower_path.contains("/auth/refresh");
    let has_grant_type =
        entry.request_form_params.iter().any(|(k, _)| {
            k.eq_ignore_ascii_case("grant_type") || k.eq_ignore_ascii_case("grantType")
        }) || entry
            .request_body_json
            .as_ref()
            .and_then(Value::as_object)
            .is_some_and(|m| m.contains_key("grant_type") || m.contains_key("grantType"));
    let has_oauth_response = entry
        .response_body_json
        .as_ref()
        .and_then(Value::as_object)
        .is_some_and(|m| {
            let has_access = m.contains_key("access_token") || m.contains_key("accessToken");
            let has_meta = m.contains_key("token_type")
                || m.contains_key("tokenType")
                || m.contains_key("refresh_token")
                || m.contains_key("refreshToken")
                || m.contains_key("expires_in")
                || m.contains_key("expiresIn");
            has_access && has_meta
        });

    (token_path && (has_grant_type || has_oauth_response)) || (has_grant_type && has_oauth_response)
}

fn entry_has_unknown_signature(entry: &NormalizedEntry) -> bool {
    entry.request_headers.iter().any(|(name, _)| {
        let lower = name.trim().to_ascii_lowercase();
        matches!(
            lower.as_str(),
            "x-signature" | "x-hmac-signature" | "x-auth-signature" | "signature"
        )
    })
}

fn is_login_request(entry: &NormalizedEntry) -> bool {
    let lower = entry.path.to_ascii_lowercase();
    let login_path = lower.contains("/login")
        || lower.contains("/signin")
        || lower.contains("/oauth/token")
        || lower.contains("/auth/token")
        || lower.ends_with("/sessions");
    let produces_auth = !entry.response_cookies.is_empty()
        || entry
            .response_body_json
            .as_ref()
            .and_then(Value::as_object)
            .is_some_and(|m| {
                m.contains_key("access_token")
                    || m.contains_key("accessToken")
                    || m.contains_key("session_token")
                    || m.contains_key("sessionToken")
                    || m.contains_key("token")
            });
    login_path && (produces_auth || entry.method == "POST")
}

fn is_refresh_request(entry: &NormalizedEntry) -> bool {
    let lower = entry.path.to_ascii_lowercase();
    if lower.contains("/refresh") {
        return true;
    }
    let form_refresh = entry.request_form_params.iter().any(|(k, v)| {
        (k.eq_ignore_ascii_case("grant_type") || k.eq_ignore_ascii_case("grantType"))
            && v.eq_ignore_ascii_case("refresh_token")
    });
    let json_refresh = entry
        .request_body_json
        .as_ref()
        .and_then(Value::as_object)
        .is_some_and(|m| {
            m.get("grant_type")
                .or_else(|| m.get("grantType"))
                .and_then(Value::as_str)
                .is_some_and(|v| v.eq_ignore_ascii_case("refresh_token"))
                || m.contains_key("refresh_token")
                || m.contains_key("refreshToken")
        });
    form_refresh || json_refresh
}

fn extract_expires_in(val: Option<&Value>) -> Option<i64> {
    let obj = val?.as_object()?;
    obj.get("expires_in")
        .or_else(|| obj.get("expiresIn"))
        .and_then(Value::as_i64)
}

#[cfg(test)]
mod tests {
    use harflow_dependencies::detect_dependencies;
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

    #[test]
    fn analyzes_oauth_refresh_rotation_expiration_and_compares_two_sessions_with_schema_drift() {
        let har_a = br#"{
          "log": {
            "version": "1.2",
            "entries": [
              {
                "startedDateTime": "2026-11-18T00:00:00Z",
                "time": 10.0,
                "request": {
                  "method": "POST",
                  "url": "https://api.example.test/oauth/token",
                  "postData": { "mimeType": "application/x-www-form-urlencoded", "text": "grant_type=client_credentials" }
                },
                "response": {
                  "status": 200,
                  "content": { "mimeType": "application/json", "text": "{\"access_token\":\"hf_synthetic_a1\",\"refresh_token\":\"hf_synthetic_ref_a1\",\"token_type\":\"Bearer\",\"expires_in\":3600}" }
                }
              },
              {
                "startedDateTime": "2026-11-18T00:00:01Z",
                "time": 8.0,
                "request": {
                  "method": "GET",
                  "url": "https://api.example.test/v1/projects?cursor=cur_a1&limit=20",
                  "headers": [
                    { "name": "Authorization", "value": "Bearer hf_synthetic_a1" },
                    { "name": "X-API-Key", "value": "hf_synthetic_apikey_1" }
                  ]
                },
                "response": {
                  "status": 200,
                  "content": { "mimeType": "application/json", "text": "{\"items\":[{\"id\":\"p1\"}],\"nextCursor\":\"cur_a2\"}" }
                }
              },
              {
                "startedDateTime": "2026-11-18T00:00:02Z",
                "time": 6.0,
                "request": {
                  "method": "GET",
                  "url": "https://api.example.test/v1/projects?cursor=cur_a2&limit=20",
                  "headers": [{ "name": "Authorization", "value": "Bearer hf_synthetic_a1" }]
                },
                "response": { "status": 401, "content": { "mimeType": "application/json", "text": "{\"error\":\"expired\"}" } }
              },
              {
                "startedDateTime": "2026-11-18T00:00:03Z",
                "time": 9.0,
                "request": {
                  "method": "POST",
                  "url": "https://api.example.test/oauth/token",
                  "postData": { "mimeType": "application/json", "text": "{\"grant_type\":\"refresh_token\",\"refresh_token\":\"hf_synthetic_ref_a1\"}" }
                },
                "response": {
                  "status": 200,
                  "content": { "mimeType": "application/json", "text": "{\"access_token\":\"hf_synthetic_a2\",\"refresh_token\":\"hf_synthetic_ref_a2\",\"token_type\":\"Bearer\",\"expires_in\":3600}" }
                }
              }
            ]
          }
        }"#;

        let har_b = br#"{
          "log": {
            "version": "1.2",
            "entries": [
              {
                "startedDateTime": "2026-11-18T01:00:00Z",
                "time": 10.0,
                "request": {
                  "method": "POST",
                  "url": "https://api.example.test/oauth/token",
                  "postData": { "mimeType": "application/x-www-form-urlencoded", "text": "grant_type=client_credentials" }
                },
                "response": {
                  "status": 200,
                  "content": { "mimeType": "application/json", "text": "{\"access_token\":\"hf_synthetic_b1\",\"refresh_token\":\"hf_synthetic_ref_b1\",\"token_type\":\"Bearer\",\"expires_in\":3600}" }
                }
              },
              {
                "startedDateTime": "2026-11-18T01:00:01Z",
                "time": 8.0,
                "request": {
                  "method": "GET",
                  "url": "https://api.example.test/v1/projects?cursor=cur_b1&limit=20",
                  "headers": [{ "name": "Authorization", "value": "Bearer hf_synthetic_b1" }]
                },
                "response": {
                  "status": 200,
                  "content": { "mimeType": "application/json", "text": "{\"items\":[{\"id\":\"p2\"}],\"nextCursor\":\"cur_b2\",\"extraDrift\":true}" }
                }
              },
              {
                "startedDateTime": "2026-11-18T01:00:02Z",
                "time": 7.0,
                "request": {
                  "method": "GET",
                  "url": "https://api.example.test/v1/audit",
                  "headers": [{ "name": "Authorization", "value": "Bearer hf_synthetic_b1" }]
                },
                "response": { "status": 200, "content": { "mimeType": "application/json", "text": "{\"events\":[]}" } }
              }
            ]
          }
        }"#;

        let sa = normalize_har_session(
            "sess-a",
            "a.har",
            &parse_har_bytes(har_a, &ImportLimits::default()).expect("a"),
        );
        let sb = normalize_har_session(
            "sess-b",
            "b.har",
            &parse_har_bytes(har_b, &ImportLimits::default()).expect("b"),
        );
        let sessions = [sa, sb];
        let endpoints = discover_endpoints(&sessions);
        let deps = detect_dependencies(&sessions, &endpoints);

        let lifecycle = analyze_session_lifecycle(&sessions, &deps);
        assert!(lifecycle
            .auth_flows
            .iter()
            .any(|f| f.scheme == AuthScheme::Bearer));
        assert!(lifecycle
            .auth_flows
            .iter()
            .any(|f| f.scheme == AuthScheme::OauthLike));
        assert!(lifecycle
            .auth_flows
            .iter()
            .any(|f| f.scheme == AuthScheme::ApiKey));
        assert!(!lifecycle.login_request_ids.is_empty());
        assert!(!lifecycle.refresh_request_ids.is_empty());
        assert!(!lifecycle.token_rotations.is_empty());
        assert!(lifecycle
            .expiration_signals
            .iter()
            .any(|s| s.kind == "http_401"));
        assert!(lifecycle
            .expiration_signals
            .iter()
            .any(|s| s.kind == "expires_in"));
        assert!(lifecycle.auto_refresh_eligible);

        let lifecycle_json = serde_json::to_string(&lifecycle).expect("json");
        for raw_secret in [
            "hf_synthetic_a1",
            "hf_synthetic_a2",
            "hf_synthetic_b1",
            "hf_synthetic_apikey_1",
        ] {
            assert!(!lifecycle_json.contains(raw_secret));
        }

        let cmp = compare_sessions(&sessions, &deps);
        assert!(cmp
            .shared_endpoints
            .contains(&"GET api.example.test /v1/projects".to_string()));
        assert!(cmp.session_only_endpoints.iter().any(|p| p.endpoint_key
            == "GET api.example.test /v1/audit"
            && p.session_id == "sess-b"));
        assert!(cmp.parameter_variability.iter().any(|p| {
            p.endpoint_key == "GET api.example.test /v1/projects"
                && p.name == "limit"
                && p.variability == VariabilityKind::Constant
        }));
        assert!(cmp.parameter_variability.iter().any(|p| {
            p.endpoint_key == "GET api.example.test /v1/projects"
                && p.name == "Authorization"
                && p.variability == VariabilityKind::SessionScoped
        }));
        assert!(cmp.parameter_variability.iter().any(|p| {
            p.endpoint_key == "GET api.example.test /v1/projects"
                && p.name == "cursor"
                && p.variability == VariabilityKind::RequestVolatile
        }));
        assert!(cmp
            .auth_differences
            .iter()
            .any(|d| d.scheme == AuthScheme::ApiKey));
        assert!(cmp.schema_drifts.iter().any(|d| {
            d.endpoint_key == "GET api.example.test /v1/projects"
                && d.response_diff
                    .as_ref()
                    .is_some_and(|rd| rd.added_fields.contains(&"extraDrift".to_string()))
        }));
    }

    #[test]
    fn detects_camelcase_oauth_tokens_and_refresh_flow() {
        let raw = br#"{
          "log": {
            "version": "1.2",
            "entries": [
              {
                "startedDateTime": "2026-11-18T00:00:00Z",
                "time": 10.0,
                "request": {
                  "method": "POST",
                  "url": "https://dummy.example.test/auth/login",
                  "postData": { "mimeType": "application/json", "text": "{\"username\":\"alice\",\"password\":\"hf_synthetic_pw\"}" }
                },
                "response": {
                  "status": 200,
                  "content": { "mimeType": "application/json", "text": "{\"accessToken\":\"hf_synthetic_at_1\",\"refreshToken\":\"hf_synthetic_rt_1\"}" }
                }
              },
              {
                "startedDateTime": "2026-11-18T00:00:01Z",
                "time": 10.0,
                "request": {
                  "method": "POST",
                  "url": "https://dummy.example.test/auth/refresh",
                  "postData": { "mimeType": "application/json", "text": "{\"refreshToken\":\"hf_synthetic_rt_1\"}" }
                },
                "response": {
                  "status": 200,
                  "content": { "mimeType": "application/json", "text": "{\"accessToken\":\"hf_synthetic_at_2\",\"refreshToken\":\"hf_synthetic_rt_2\"}" }
                }
              }
            ]
          }
        }"#;

        let har = parse_har_bytes(raw, &ImportLimits::default()).expect("parse");
        let s = normalize_har_session("sess-1", "dummy.har", &har);
        let endpoints = discover_endpoints(std::slice::from_ref(&s));
        let deps = detect_dependencies(std::slice::from_ref(&s), &endpoints);
        let lifecycle = analyze_session_lifecycle(std::slice::from_ref(&s), &deps);

        assert!(lifecycle
            .auth_flows
            .iter()
            .any(|f| f.scheme == AuthScheme::OauthLike));
        assert!(lifecycle.auto_refresh_eligible);
        assert!(!lifecycle.token_rotations.is_empty());
    }
}
