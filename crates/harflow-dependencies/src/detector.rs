use std::collections::{BTreeMap, BTreeSet, HashMap};

use base64::engine::general_purpose::{STANDARD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use harflow_model::{
    Dependency, DependencyType, Endpoint, Evidence, Transformation, ValueLocation, ValueReference,
};
use harflow_normalizer::NormalizedSession;

use crate::extractor::{
    extract_timeline_candidates, CandidateValue, EntryCandidates, ScalarTypeHint,
};
use crate::timeline::build_session_timelines;
use crate::{classify_confidence, score_confidence, ConfidenceFactors};

const MAX_EVIDENCE_PER_DEPENDENCY: usize = 10;
const MAX_BUCKET_SLOTS: usize = 4;

const WEAK_SCALARS: &[&str] = &[
    "true",
    "false",
    "null",
    "none",
    "undefined",
    "nil",
    "nan",
    "ok",
    "success",
    "fail",
    "failed",
    "error",
    "active",
    "inactive",
    "pending",
    "completed",
    "done",
    "enabled",
    "disabled",
    "asc",
    "desc",
    "json",
    "xml",
    "html",
    "text",
    "utf-8",
    "utf8",
    "en",
    "ru",
    "uk",
    "de",
    "fr",
    "usd",
    "eur",
    "uah",
    "gbp",
    "bearer",
    "basic",
    "digest",
    "get",
    "post",
    "put",
    "patch",
    "delete",
    "admin",
    "user",
    "guest",
    "default",
    "auto",
    "normal",
    "high",
    "low",
    "pro",
    "free",
    "v1",
    "v2",
    "v3",
];

const WEAK_PRODUCER_KEYS: &[&str] = &[
    "limit",
    "per_page",
    "pagesize",
    "page_size",
    "size",
    "total",
    "total_count",
    "count",
    "status",
    "code",
    "ok",
    "success",
    "version",
    "message",
    "msg",
    "method",
    "currency",
    "locale",
    "lang",
    "language",
    "role",
];

const IGNORED_CONSUMER_PAGINATION_CONFIG_KEYS: &[&str] = &[
    "limit",
    "per_page",
    "pagesize",
    "page_size",
    "size",
    "sort",
    "order",
    "dir",
    "direction",
];

/// Runs linear-time per-session dependency detection across sanitized normalized sessions.
#[must_use]
pub fn detect_dependencies(
    sessions: &[NormalizedSession],
    endpoints: &[Endpoint],
) -> Vec<Dependency> {
    if sessions.is_empty() {
        return Vec::new();
    }

    let timelines = build_session_timelines(sessions);
    let endpoint_sessions = collect_endpoint_sessions(endpoints, sessions);

    let mut aggregated: BTreeMap<AggregationKey, AggregatedDependency> = BTreeMap::new();

    for tl in &timelines {
        let entries = extract_timeline_candidates(tl, endpoints);
        detect_within_session(tl.session_id, &entries, &endpoint_sessions, &mut aggregated);
    }

    let mut dependencies: Vec<Dependency> = aggregated
        .into_iter()
        .map(|(key, agg)| agg.finalize(&key, &endpoint_sessions))
        .collect();

    dependencies.sort_by(|a, b| {
        b.confidence
            .total_cmp(&a.confidence)
            .then_with(|| a.id.cmp(&b.id))
    });

    dependencies
}

fn collect_endpoint_sessions(
    endpoints: &[Endpoint],
    sessions: &[NormalizedSession],
) -> HashMap<String, BTreeSet<String>> {
    let mut req_to_session: HashMap<&str, &str> = HashMap::new();
    let mut map: HashMap<String, BTreeSet<String>> = HashMap::new();

    for s in sessions {
        for e in &s.entries {
            req_to_session.insert(e.id.as_str(), s.session_id.as_str());
        }
    }

    for ep in endpoints {
        let ep_key = format!("{} {} {}", ep.method, ep.host, ep.path_template);
        let entry = map.entry(ep_key).or_default();
        for req_id in &ep.observed_request_ids {
            if let Some(&sid) = req_to_session.get(req_id.as_str()) {
                entry.insert(sid.to_string());
            }
        }
    }

    for s in sessions {
        for e in &s.entries {
            let fallback_key = format!("{} {} {}", e.method, e.host, e.path);
            map.entry(fallback_key)
                .or_default()
                .insert(s.session_id.clone());
        }
    }

    map
}

#[derive(Default)]
struct ProducerBucket {
    earliest_mutating: Option<usize>,
    latest_auth_cookie: Option<usize>,
    latest: Option<usize>,
    recent: [Option<usize>; MAX_BUCKET_SLOTS],
    recent_len: usize,
}

impl ProducerBucket {
    fn push(&mut self, idx: usize, prod: &CandidateValue) {
        if self.earliest_mutating.is_none()
            && matches!(prod.method.as_str(), "POST" | "PUT")
            && matches!(prod.response_status, 200..=202)
        {
            self.earliest_mutating = Some(idx);
        }
        if matches!(prod.location, ValueLocation::SetCookie) || is_auth_or_cursor_producer(prod) {
            self.latest_auth_cookie = Some(idx);
        }
        self.latest = Some(idx);

        if self.recent_len < MAX_BUCKET_SLOTS {
            self.recent[self.recent_len] = Some(idx);
            self.recent_len += 1;
        } else {
            self.recent.rotate_left(1);
            self.recent[MAX_BUCKET_SLOTS - 1] = Some(idx);
        }
    }

    fn candidate_indices(&self) -> [Option<usize>; MAX_BUCKET_SLOTS + 3] {
        [
            self.latest_auth_cookie,
            self.earliest_mutating,
            self.latest,
            self.recent[0],
            self.recent[1],
            self.recent[2],
            self.recent[3],
        ]
    }
}

#[derive(Default)]
struct SessionIndex {
    producers: Vec<CandidateValue>,
    exact: HashMap<String, ProducerBucket>,
    fingerprint: HashMap<String, ProducerBucket>,
    url_decoded: HashMap<String, ProducerBucket>,
    b64_decoded: HashMap<String, ProducerBucket>,
    producer_freq: HashMap<String, usize>,
}

impl SessionIndex {
    fn insert_producer(&mut self, prod: CandidateValue) {
        let idx = self.producers.len();

        *self.producer_freq.entry(prod.value.clone()).or_insert(0) += 1;

        if let Some((_, fp)) = &prod.redacted_marker {
            self.fingerprint
                .entry(fp.clone())
                .or_default()
                .push(idx, &prod);
        } else {
            self.exact
                .entry(prod.value.clone())
                .or_default()
                .push(idx, &prod);

            if let Some(fp) = &prod.plain_fingerprint {
                self.fingerprint
                    .entry(fp.clone())
                    .or_default()
                    .push(idx, &prod);
            }

            if prod.value.contains('%') {
                if let Some(decoded) = percent_decode(&prod.value, false) {
                    if decoded != prod.value && !decoded.is_empty() {
                        self.url_decoded
                            .entry(decoded)
                            .or_default()
                            .push(idx, &prod);
                    }
                }
            }

            if prod.value.len() >= 8 {
                if let Some(decoded) = try_decode_base64_text(&prod.value) {
                    self.b64_decoded
                        .entry(decoded)
                        .or_default()
                        .push(idx, &prod);
                }
            }
        }

        self.producers.push(prod);
    }
}

fn detect_within_session(
    session_id: &str,
    entries: &[EntryCandidates],
    _endpoint_sessions: &HashMap<String, BTreeSet<String>>,
    aggregated: &mut BTreeMap<AggregationKey, AggregatedDependency>,
) {
    let mut first_client_val: HashMap<String, usize> = HashMap::new();
    let mut first_client_fp: HashMap<String, usize> = HashMap::new();

    for entry in entries {
        for cons in &entry.consumers {
            if let Some((_, fp)) = &cons.redacted_marker {
                first_client_fp
                    .entry(fp.clone())
                    .or_insert(cons.timeline_pos);
            } else {
                first_client_val
                    .entry(cons.value.clone())
                    .or_insert(cons.timeline_pos);
                if let Some(fp) = &cons.plain_fingerprint {
                    first_client_fp
                        .entry(fp.clone())
                        .or_insert(cons.timeline_pos);
                }
            }
        }
    }

    let mut index = SessionIndex::default();

    for entry in entries {
        // 1. Evaluate current request consumers strictly against previously indexed producers (T > 0).
        for cons in &entry.consumers {
            if let Some(matched) = find_best_match(cons, &index) {
                record_match(session_id, matched, cons, &index, aggregated);
            }
        }

        // 2. Index current entry's response producers for strictly later requests.
        for prod in &entry.producers {
            if is_weak_producer_candidate(prod) {
                continue;
            }
            if is_preexisting_client_value(prod, &first_client_val, &first_client_fp) {
                continue;
            }
            index.insert_producer(prod.clone());
        }
    }
}

fn is_preexisting_client_value(
    prod: &CandidateValue,
    first_client_val: &HashMap<String, usize>,
    first_client_fp: &HashMap<String, usize>,
) -> bool {
    if let Some((_, fp)) = &prod.redacted_marker {
        if let Some(&c_pos) = first_client_fp.get(fp) {
            return c_pos <= prod.timeline_pos;
        }
        return false;
    }

    if let Some(&c_pos) = first_client_val.get(&prod.value) {
        if c_pos <= prod.timeline_pos {
            return true;
        }
    }
    if let Some(fp) = &prod.plain_fingerprint {
        if let Some(&c_pos) = first_client_fp.get(fp) {
            if c_pos <= prod.timeline_pos {
                return true;
            }
        }
    }
    false
}

struct MatchedCandidate<'a> {
    prod: &'a CandidateValue,
    transformation: Transformation,
    dep_type: DependencyType,
    value_fidelity: f64,
    structural_score: f64,
}

fn find_best_match<'a>(
    cons: &CandidateValue,
    index: &'a SessionIndex,
) -> Option<MatchedCandidate<'a>> {
    if is_ignored_consumer(cons) {
        return None;
    }

    let mut candidates: Vec<(usize, Transformation, f64)> = Vec::new();

    if let Some((_, fp)) = &cons.redacted_marker {
        if let Some(bucket) = index.fingerprint.get(fp) {
            collect_bucket_candidates(bucket, &index.producers, cons, None, &mut candidates);
        }
    } else {
        if let Some(bucket) = index.exact.get(&cons.value) {
            collect_bucket_candidates(
                bucket,
                &index.producers,
                cons,
                Some(Transformation::Identity),
                &mut candidates,
            );
        }

        if let Some(fp) = &cons.plain_fingerprint {
            if let Some(bucket) = index.fingerprint.get(fp) {
                collect_bucket_candidates(
                    bucket,
                    &index.producers,
                    cons,
                    Some(Transformation::Identity),
                    &mut candidates,
                );
            }
        }

        // PrefixSuffix unredacted header values like "Bearer <token>" or "Token <token>".
        if let Some(stripped) = strip_auth_or_token_prefix(&cons.value) {
            if stripped.len() >= 4 {
                if let Some(bucket) = index.exact.get(stripped) {
                    collect_bucket_candidates(
                        bucket,
                        &index.producers,
                        cons,
                        Some(Transformation::PrefixSuffix),
                        &mut candidates,
                    );
                }
            }
        }

        // UrlEncoding and URL-encoded JsonExtract.
        let plus_as_space = matches!(cons.location, ValueLocation::Query | ValueLocation::Body);
        if cons.value.contains('%') || (plus_as_space && cons.value.contains('+')) {
            if let Some(decoded) = percent_decode(&cons.value, plus_as_space) {
                if decoded != cons.value && decoded.len() >= 2 {
                    if let Some(bucket) = index.exact.get(&decoded) {
                        collect_bucket_candidates(
                            bucket,
                            &index.producers,
                            cons,
                            Some(Transformation::UrlEncoding),
                            &mut candidates,
                        );
                    }
                    try_json_extract_lookup(&decoded, cons, index, &mut candidates);
                }
            }
        } else if cons.value.contains(' ') {
            // Producer may have had %20 / %2F encoded while normalizer decoded consumer query.
            if let Some(bucket) = index.url_decoded.get(&cons.value) {
                collect_bucket_candidates(
                    bucket,
                    &index.producers,
                    cons,
                    Some(Transformation::UrlEncoding),
                    &mut candidates,
                );
            }
        }

        // Direct JsonExtract (e.g., filter={"userId":"usr_42"}).
        try_json_extract_lookup(&cons.value, cons, index, &mut candidates);

        // Base64 (both directions).
        if cons.value.len() >= 8 {
            if let Some(decoded) = try_decode_base64_text(&cons.value) {
                if let Some(bucket) = index.exact.get(&decoded) {
                    collect_bucket_candidates(
                        bucket,
                        &index.producers,
                        cons,
                        Some(Transformation::Base64),
                        &mut candidates,
                    );
                }
            }
        }
        if let Some(bucket) = index.b64_decoded.get(&cons.value) {
            collect_bucket_candidates(
                bucket,
                &index.producers,
                cons,
                Some(Transformation::Base64),
                &mut candidates,
            );
        }
    }

    let mut best: Option<(f64, MatchedCandidate<'a>)> = None;

    for (prod_idx, base_trans, rank_bonus) in candidates {
        let prod = &index.producers[prod_idx];
        if prod.timeline_pos >= cons.timeline_pos || prod.request_id == cons.request_id {
            continue;
        }
        if !passes_pair_weak_filter(prod, cons, &base_trans) {
            continue;
        }
        let Some((trans, fidelity)) = resolve_transformation(prod, cons, base_trans) else {
            continue;
        };
        let dep_type = classify_dependency_type(prod, cons, &trans);
        let structural = score_structural_compatibility(prod, cons, &dep_type);
        if structural <= 0.0 {
            continue;
        }

        let priority = selection_priority(prod, cons, &dep_type, structural, fidelity, rank_bonus);
        let replace = match &best {
            None => true,
            Some((best_prio, _)) => priority > *best_prio,
        };
        if replace {
            best = Some((
                priority,
                MatchedCandidate {
                    prod,
                    transformation: trans,
                    dep_type,
                    value_fidelity: fidelity,
                    structural_score: structural,
                },
            ));
        }
    }

    best.map(|(_, m)| m)
}

fn try_json_extract_lookup(
    raw: &str,
    cons: &CandidateValue,
    index: &SessionIndex,
    out: &mut Vec<(usize, Transformation, f64)>,
) {
    let trimmed = raw.trim();
    if !((trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']')))
    {
        return;
    }
    let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return;
    };
    let mut scalars = Vec::new();
    collect_json_filter_scalars(&val, 0, &mut scalars);
    for scalar in scalars {
        if let Some(bucket) = index.exact.get(&scalar) {
            collect_bucket_candidates(
                bucket,
                &index.producers,
                cons,
                Some(Transformation::JsonExtract),
                out,
            );
        }
    }
}

fn collect_json_filter_scalars(val: &serde_json::Value, depth: usize, out: &mut Vec<String>) {
    if depth > 6 {
        return;
    }
    match val {
        serde_json::Value::Object(map) => {
            for v in map.values() {
                collect_json_filter_scalars(v, depth + 1, out);
            }
        }
        serde_json::Value::Array(items) => {
            for v in items.iter().take(15) {
                collect_json_filter_scalars(v, depth + 1, out);
            }
        }
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.len() >= 4 && !WEAK_SCALARS.contains(&trimmed.to_ascii_lowercase().as_str())
            {
                out.push(trimmed.to_string());
            }
        }
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i >= 100 {
                    out.push(i.to_string());
                }
            }
        }
        serde_json::Value::Bool(_) | serde_json::Value::Null => {}
    }
}

fn collect_bucket_candidates(
    bucket: &ProducerBucket,
    producers: &[CandidateValue],
    cons: &CandidateValue,
    forced_trans: Option<Transformation>,
    out: &mut Vec<(usize, Transformation, f64)>,
) {
    let mut seen = [usize::MAX; MAX_BUCKET_SLOTS + 3];
    let mut seen_len = 0usize;

    for (slot_rank, maybe_idx) in bucket.candidate_indices().into_iter().enumerate() {
        let Some(idx) = maybe_idx else {
            continue;
        };
        if seen[..seen_len].contains(&idx) {
            continue;
        }
        seen[seen_len] = idx;
        seen_len += 1;

        let prod = &producers[idx];
        let trans = forced_trans.clone().unwrap_or_else(|| {
            if cons.redacted_marker.is_some()
                && prod.redacted_marker.is_some()
                && is_bearer_header_pair(prod, cons)
            {
                Transformation::PrefixSuffix
            } else {
                Transformation::Identity
            }
        });
        let rank_bonus = if slot_rank == 0 { 0.02 } else { 0.0 };
        out.push((idx, trans, rank_bonus));
    }
}

fn resolve_transformation(
    prod: &CandidateValue,
    cons: &CandidateValue,
    base: Transformation,
) -> Option<(Transformation, f64)> {
    if prod.derived_prefix_suffix {
        return Some((Transformation::PrefixSuffix, 0.95));
    }
    if is_bearer_header_pair(prod, cons) {
        return Some((Transformation::PrefixSuffix, 0.95));
    }
    if prod.redacted_marker.is_some() && cons.redacted_marker.is_some() {
        if matches!(
            (&prod.location, &cons.location),
            (ValueLocation::SetCookie, ValueLocation::Cookie)
        ) {
            if prod.normalized_key != cons.normalized_key {
                return None;
            }
        } else if !are_compatible_redacted_names(prod, cons) {
            return None;
        }
    }

    let fidelity = match base {
        Transformation::Identity => 1.0,
        Transformation::PrefixSuffix | Transformation::UrlEncoding => 0.95,
        Transformation::JsonExtract => 0.92,
        Transformation::Base64 => 0.90,
        Transformation::Custom => 0.85,
    };
    Some((base, fidelity))
}

fn are_compatible_redacted_names(prod: &CandidateValue, cons: &CandidateValue) -> bool {
    let pk = leaf_key(&prod.normalized_key);
    let ck = leaf_key(&cons.normalized_key);
    if pk == ck {
        return true;
    }
    if is_csrf_key(pk) && is_csrf_key(ck) {
        return true;
    }
    if is_refresh_key(pk) && is_refresh_key(ck) {
        return true;
    }
    if is_api_key_name(pk) && is_api_key_name(ck) {
        return true;
    }
    if is_session_token_key(pk) && (is_bearer_header_key(ck) || is_session_token_key(ck)) {
        return true;
    }
    false
}

fn is_bearer_header_pair(prod: &CandidateValue, cons: &CandidateValue) -> bool {
    let pk = leaf_key(&prod.normalized_key);
    is_bearer_header_key(&cons.normalized_key)
        && (is_session_token_key(pk)
            || prod.redacted_marker.as_ref().is_some_and(|(kind, _)| {
                matches!(kind.as_str(), "access_token" | "id_token" | "jwt_token")
            }))
}

fn is_bearer_header_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "authorization" | "proxy-authorization"
    )
}

fn is_weak_producer_candidate(prod: &CandidateValue) -> bool {
    if prod.redacted_marker.is_some() {
        return false;
    }

    let leaf = leaf_key(&prod.normalized_key);
    if WEAK_PRODUCER_KEYS.contains(&leaf) {
        return true;
    }

    is_weak_scalar_value(&prod.value, prod.type_hint)
}

fn is_ignored_consumer(cons: &CandidateValue) -> bool {
    if cons.redacted_marker.is_some() {
        return false;
    }

    if matches!(cons.location, ValueLocation::Query)
        && IGNORED_CONSUMER_PAGINATION_CONFIG_KEYS.contains(&cons.normalized_key.as_str())
    {
        return true;
    }

    is_weak_scalar_value(&cons.value, cons.type_hint)
}

fn is_weak_scalar_value(val: &str, hint: ScalarTypeHint) -> bool {
    let trimmed = val.trim();
    let lower = trimmed.to_ascii_lowercase();
    if trimmed.is_empty() || WEAK_SCALARS.contains(&lower.as_str()) {
        return true;
    }

    match hint {
        ScalarTypeHint::Redacted => false,
        ScalarTypeHint::Uuid => false,
        ScalarTypeHint::Number => true, // Exclude raw float metrics/prices.
        ScalarTypeHint::Integer => match trimmed.parse::<i64>() {
            Ok(n) => (0..=9).contains(&n) || n < 0,
            Err(_) => true,
        },
        ScalarTypeHint::String => trimmed.len() < 2,
    }
}

fn passes_pair_weak_filter(
    prod: &CandidateValue,
    cons: &CandidateValue,
    trans: &Transformation,
) -> bool {
    if prod.redacted_marker.is_some() || cons.redacted_marker.is_some() {
        return true;
    }

    if !matches!(trans, Transformation::Identity) {
        return true;
    }

    let pk = leaf_key(&prod.normalized_key);
    let ck = leaf_key(&cons.normalized_key);

    if prod.type_hint == ScalarTypeHint::Integer {
        if let Ok(n) = prod.value.parse::<i64>() {
            if (10..=99).contains(&n) {
                let both_id = (is_id_key(pk) || prod.derived_prefix_suffix)
                    && (is_id_key(ck) || matches!(cons.location, ValueLocation::Path));
                if !both_id {
                    return false;
                }
            }
        }
    }

    if prod.type_hint == ScalarTypeHint::String {
        let v = prod.value.trim();
        let all_alpha = v.bytes().all(|b| b.is_ascii_alphabetic());
        if all_alpha && v.len() <= 4 {
            let explicit_id_or_token = (is_id_key(pk) || is_token_or_cursor_key(pk))
                && (is_id_key(ck)
                    || is_token_or_cursor_key(ck)
                    || matches!(cons.location, ValueLocation::Path));
            if !explicit_id_or_token {
                return false;
            }
        }
    }

    true
}

fn classify_dependency_type(
    prod: &CandidateValue,
    cons: &CandidateValue,
    trans: &Transformation,
) -> DependencyType {
    if matches!(
        (&prod.location, &cons.location),
        (ValueLocation::SetCookie, ValueLocation::Cookie)
    ) {
        return DependencyType::SetCookieToCookie;
    }

    if is_pagination_flow(prod, cons) {
        return DependencyType::PaginationChain;
    }

    if is_session_chain_flow(prod, cons) {
        return DependencyType::SessionChain;
    }

    if matches!(
        trans,
        Transformation::Base64 | Transformation::JsonExtract | Transformation::UrlEncoding
    ) && matches!(
        prod.location,
        ValueLocation::ResponseBody | ValueLocation::ResponseHeader
    ) && matches!(cons.location, ValueLocation::Query)
        && is_pagination_key(leaf_key(&prod.normalized_key))
    {
        return DependencyType::PaginationChain;
    }

    match cons.location {
        ValueLocation::Header => DependencyType::ResponseToHeader,
        ValueLocation::Query => DependencyType::ResponseToQuery,
        ValueLocation::Body => DependencyType::ResponseToBody,
        ValueLocation::Path => DependencyType::ResponseToPath,
        ValueLocation::Cookie => DependencyType::SetCookieToCookie,
        _ => DependencyType::DerivedValue,
    }
}

fn selection_priority(
    prod: &CandidateValue,
    cons: &CandidateValue,
    dep_type: &DependencyType,
    structural: f64,
    fidelity: f64,
    rank_bonus: f64,
) -> f64 {
    let mut prio = structural * 2.0 + fidelity + rank_bonus;
    match dep_type {
        DependencyType::PaginationChain
        | DependencyType::SetCookieToCookie
        | DependencyType::SessionChain => {
            // Prefer most recent preceding producer for cursors/cookies/rotated tokens.
            prio += (prod.timeline_pos as f64) * 1e-5;
        }
        _ => {
            if matches!(prod.method.as_str(), "POST" | "PUT")
                && matches!(prod.response_status, 200..=202)
            {
                prio += 0.25;
            }
            if prod.endpoint_key != cons.endpoint_key {
                prio += 0.05;
            }
        }
    }
    prio
}

fn score_structural_compatibility(
    prod: &CandidateValue,
    cons: &CandidateValue,
    dep_type: &DependencyType,
) -> f64 {
    match dep_type {
        DependencyType::SetCookieToCookie => {
            if prod.normalized_key == cons.normalized_key {
                1.0
            } else {
                0.0
            }
        }
        DependencyType::SessionChain => 1.0,
        DependencyType::PaginationChain => 1.0,
        _ => {
            if prod.derived_prefix_suffix
                && matches!(
                    cons.location,
                    ValueLocation::Path | ValueLocation::Query | ValueLocation::Body
                )
            {
                return 0.95;
            }

            let pk = leaf_key(&prod.normalized_key);
            let ck = leaf_key(&cons.normalized_key);

            let p_tokens = split_key_tokens(pk);
            let c_tokens = split_key_tokens(ck);
            if p_tokens == c_tokens && !p_tokens.is_empty() {
                return if p_tokens == ["id"] && !shares_endpoint_noun(prod, cons) {
                    0.80
                } else {
                    1.0
                };
            }

            if matches!(cons.location, ValueLocation::Path) && pk == "id" {
                let prod_noun = extract_endpoint_resource_noun(&prod.endpoint_key);
                if let Some(noun) = prod_noun {
                    if c_tokens.iter().any(|t| t == &noun) {
                        return 0.95;
                    }
                }
                if shares_endpoint_noun(prod, cons) {
                    return 0.95;
                }
            }

            if (pk == "id" || p_tokens.contains(&"id".to_string()))
                && c_tokens.contains(&"id".to_string())
            {
                let prod_noun = extract_endpoint_resource_noun(&prod.endpoint_key);
                if prod_noun.as_ref().is_some_and(|n| c_tokens.contains(n)) {
                    return 0.95;
                }
                if shares_structural_prefix(&prod.normalized_key, &c_tokens) {
                    return 0.90;
                }
                return 0.80;
            }

            if p_tokens
                .iter()
                .any(|t| t.len() >= 3 && t != "id" && c_tokens.contains(t))
            {
                return 0.88;
            }

            if (is_id_key(pk) || is_token_or_cursor_key(pk))
                && (is_id_key(ck)
                    || is_token_or_cursor_key(ck)
                    || matches!(cons.location, ValueLocation::Path))
            {
                return 0.72;
            }

            0.45
        }
    }
}

fn shares_structural_prefix(norm_path: &str, c_tokens: &[String]) -> bool {
    norm_path
        .split(['.', '['])
        .filter(|seg| !seg.is_empty() && *seg != "$" && !seg.contains('*'))
        .any(|seg| {
            let st = singular_stem(&seg.to_ascii_lowercase());
            st.len() >= 3 && c_tokens.iter().any(|ct| ct == &st)
        })
}

fn shares_endpoint_noun(prod: &CandidateValue, cons: &CandidateValue) -> bool {
    matches!(
        (
            extract_endpoint_resource_noun(&prod.endpoint_key),
            extract_endpoint_resource_noun(&cons.endpoint_key)
        ),
        (Some(a), Some(b)) if a == b
    )
}

fn extract_endpoint_resource_noun(endpoint_key: &str) -> Option<String> {
    let path = endpoint_key.split_whitespace().nth(2)?;
    let static_segs: Vec<&str> = path
        .split('/')
        .filter(|s| !s.is_empty() && !s.starts_with('{') && !is_version_seg(s))
        .collect();
    static_segs
        .last()
        .map(|s| singular_stem(&s.to_ascii_lowercase()))
}

fn is_version_seg(seg: &str) -> bool {
    let lower = seg.to_ascii_lowercase();
    (lower.starts_with('v') && lower[1..].bytes().all(|b| b.is_ascii_digit())) || lower == "api"
}

fn singular_stem(s: &str) -> String {
    if s.ends_with("ies") && s.len() > 4 {
        format!("{}y", &s[..s.len() - 3])
    } else if s.ends_with('s') && !s.ends_with("ss") && s.len() > 2 {
        s[..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn split_key_tokens(key: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in key.chars() {
        if !ch.is_ascii_alphanumeric() {
            if !current.is_empty() {
                tokens.push(current.to_ascii_lowercase());
                current.clear();
            }
            continue;
        }
        if ch.is_ascii_uppercase() && !current.is_empty() {
            tokens.push(current.to_ascii_lowercase());
            current.clear();
        }
        current.push(ch);
    }
    if !current.is_empty() {
        tokens.push(current.to_ascii_lowercase());
    }
    tokens
}

fn leaf_key(norm_key: &str) -> &str {
    norm_key
        .rsplit('.')
        .next()
        .unwrap_or(norm_key)
        .trim_end_matches("[*]")
}

fn is_id_key(leaf: &str) -> bool {
    let lower = leaf.to_ascii_lowercase();
    lower == "id"
        || lower.ends_with("_id")
        || lower.ends_with("id")
        || lower.ends_with("slug")
        || lower.ends_with("uuid")
}

fn is_token_or_cursor_key(leaf: &str) -> bool {
    let lower = leaf.to_ascii_lowercase();
    lower.contains("token")
        || lower.contains("cursor")
        || lower.contains("csrf")
        || lower.contains("xsrf")
        || lower.contains("secret")
        || lower.contains("key")
        || lower == "after"
        || lower == "authorization"
}

fn is_pagination_key(leaf: &str) -> bool {
    let lower = leaf.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "cursor"
            | "nextcursor"
            | "next_cursor"
            | "after"
            | "next"
            | "page_token"
            | "next_page_token"
            | "continuation_token"
            | "next_page"
            | "nextpage"
            | "next_offset"
            | "nextoffset"
    )
}

fn is_pagination_flow(prod: &CandidateValue, cons: &CandidateValue) -> bool {
    let pk = leaf_key(&prod.normalized_key);
    let ck = leaf_key(&cons.normalized_key);
    (is_pagination_key(pk) || is_pagination_key(ck))
        && matches!(cons.location, ValueLocation::Query | ValueLocation::Body)
        && prod.endpoint_key == cons.endpoint_key
}

fn is_session_chain_flow(prod: &CandidateValue, cons: &CandidateValue) -> bool {
    let pk = leaf_key(&prod.normalized_key);
    let ck = leaf_key(&cons.normalized_key);
    is_bearer_header_pair(prod, cons)
        || (is_csrf_key(pk) || is_csrf_key(ck))
        || (is_refresh_key(pk) || is_refresh_key(ck))
        || (is_api_key_name(pk) && is_api_key_name(ck))
        || (is_session_token_key(pk) && (is_bearer_header_key(ck) || is_session_token_key(ck)))
}

fn is_auth_or_cursor_producer(prod: &CandidateValue) -> bool {
    let pk = leaf_key(&prod.normalized_key);
    is_pagination_key(pk) || is_session_token_key(pk) || is_csrf_key(pk) || is_refresh_key(pk)
}

fn is_csrf_key(leaf: &str) -> bool {
    let lower = leaf.to_ascii_lowercase();
    lower.contains("csrf") || lower.contains("xsrf")
}

fn is_refresh_key(leaf: &str) -> bool {
    leaf.to_ascii_lowercase().contains("refresh")
}

fn is_api_key_name(leaf: &str) -> bool {
    let norm = leaf.to_ascii_lowercase().replace('-', "_");
    norm == "api_key" || norm == "apikey" || norm.ends_with("_api_key")
}

fn is_session_token_key(leaf: &str) -> bool {
    let norm = leaf.to_ascii_lowercase().replace('-', "_");
    matches!(
        norm.as_str(),
        "access_token"
            | "id_token"
            | "jwt_token"
            | "bearer_token"
            | "auth_token"
            | "session_token"
            | "session_id"
            | "token"
            | "jwt"
            | "authorization"
    )
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct AggregationKey {
    source_endpoint_key: String,
    source_location_tag: String,
    source_norm_key: String,
    target_endpoint_key: String,
    target_location_tag: String,
    target_norm_key: String,
    dep_type_tag: String,
    trans_tag: String,
}

struct AggregatedDependency {
    representative_source: ValueReference,
    representative_target: ValueReference,
    dependency_type: DependencyType,
    transformation: Transformation,
    evidence: Vec<Evidence>,
    seen_evidence_keys: HashMap<(String, String, String), ()>,
    observed_session_ids: BTreeSet<String>,
    observation_count: usize,
    fidelity_sum: f64,
    uniqueness_sum: f64,
    structural_sum: f64,
}

#[allow(clippy::too_many_arguments)]
fn record_match(
    session_id: &str,
    matched: MatchedCandidate<'_>,
    cons: &CandidateValue,
    index: &SessionIndex,
    aggregated: &mut BTreeMap<AggregationKey, AggregatedDependency>,
) {
    let key = AggregationKey {
        source_endpoint_key: matched.prod.endpoint_key.clone(),
        source_location_tag: format!("{:?}", matched.prod.location),
        source_norm_key: matched.prod.normalized_key.clone(),
        target_endpoint_key: cons.endpoint_key.clone(),
        target_location_tag: format!("{:?}", cons.location),
        target_norm_key: cons.normalized_key.clone(),
        dep_type_tag: format!("{:?}", matched.dep_type),
        trans_tag: format!("{:?}", matched.transformation),
    };

    let uniqueness = compute_candidate_uniqueness(matched.prod, index);

    let entry = aggregated
        .entry(key)
        .or_insert_with(|| AggregatedDependency {
            representative_source: ValueReference {
                session_id: matched.prod.session_id.clone(),
                request_id: matched.prod.request_id.clone(),
                location: matched.prod.location.clone(),
                json_path_or_key: matched.prod.path_or_key.clone(),
            },
            representative_target: ValueReference {
                session_id: cons.session_id.clone(),
                request_id: cons.request_id.clone(),
                location: cons.location.clone(),
                json_path_or_key: cons.path_or_key.clone(),
            },
            dependency_type: matched.dep_type.clone(),
            transformation: matched.transformation.clone(),
            evidence: Vec::new(),
            seen_evidence_keys: HashMap::new(),
            observed_session_ids: BTreeSet::new(),
            observation_count: 0,
            fidelity_sum: 0.0,
            uniqueness_sum: 0.0,
            structural_sum: 0.0,
        });

    entry.observed_session_ids.insert(session_id.to_string());
    entry.observation_count += 1;
    entry.fidelity_sum += matched.value_fidelity;
    entry.uniqueness_sum += uniqueness;
    entry.structural_sum += matched.structural_score;

    let ev_key = (
        cons.request_id.clone(),
        matched.prod.request_id.clone(),
        cons.path_or_key.clone(),
    );
    if entry.evidence.len() < MAX_EVIDENCE_PER_DEPENDENCY
        && !entry.seen_evidence_keys.contains_key(&ev_key)
    {
        entry.seen_evidence_keys.insert(ev_key, ());
        let observed_at = if cons.started_date_time.is_empty() {
            matched.prod.started_date_time.clone()
        } else {
            cons.started_date_time.clone()
        };
        entry.evidence.push(Evidence {
            session_id: session_id.to_string(),
            request_id: cons.request_id.clone(),
            rule_id: rule_id_for(&matched.dep_type, &matched.transformation),
            description: format!(
                "{} {:?}:{} -> {} {:?}:{} via {:?}",
                matched.prod.request_id,
                matched.prod.location,
                matched.prod.path_or_key,
                cons.request_id,
                cons.location,
                cons.path_or_key,
                matched.transformation
            ),
            observed_at,
        });
    }
}

impl AggregatedDependency {
    fn finalize(
        self,
        key: &AggregationKey,
        endpoint_sessions: &HashMap<String, BTreeSet<String>>,
    ) -> Dependency {
        let count = self.observation_count.max(1) as f64;
        let v = self.fidelity_sum / count;
        let t = 1.0;
        let u = self.uniqueness_sum / count;
        let s = self.structural_sum / count;

        let src_sessions = endpoint_sessions.get(&key.source_endpoint_key);
        let dst_sessions = endpoint_sessions.get(&key.target_endpoint_key);
        let eligible_sessions = match (src_sessions, dst_sessions) {
            (Some(a), Some(b)) => a.intersection(b).count().max(1),
            _ => 1,
        };

        let obs_sessions = self.observed_session_ids.len().max(1);
        let r = if obs_sessions >= 2 {
            let ratio =
                (obs_sessions as f64 / eligible_sessions.max(obs_sessions) as f64).clamp(0.0, 1.0);
            0.85 + 0.15 * ratio
        } else if eligible_sessions >= 2 {
            // Contradicted by other sessions that ran both endpoints without showing this dependency.
            0.35
        } else if self.observation_count >= 2
            || matches!(
                self.dependency_type,
                DependencyType::SetCookieToCookie
                    | DependencyType::SessionChain
                    | DependencyType::PaginationChain
            )
        {
            0.75
        } else if u >= 0.85 && s >= 0.85 {
            0.70
        } else {
            0.50
        };

        let confidence = score_confidence(&ConfidenceFactors {
            value_match: v,
            temporal_order: t,
            uniqueness: u,
            cross_session_repeatability: r,
            structural_compatibility: s,
        });
        let status = classify_confidence(confidence);

        let canonical_sig = format!(
            "{}|{}|{}|{}|{}|{}|{}|{}",
            key.source_endpoint_key,
            key.source_location_tag,
            key.source_norm_key,
            key.target_endpoint_key,
            key.target_location_tag,
            key.target_norm_key,
            key.dep_type_tag,
            key.trans_tag
        );
        let id = format!("dep-{:08x}", fnv1a_32(canonical_sig.as_bytes()));

        Dependency {
            id,
            source: self.representative_source,
            target: self.representative_target,
            dependency_type: self.dependency_type,
            transformation: Some(self.transformation),
            confidence,
            status,
            evidence: self.evidence,
            observed_session_ids: self.observed_session_ids.into_iter().collect(),
        }
    }
}

fn compute_candidate_uniqueness(prod: &CandidateValue, index: &SessionIndex) -> f64 {
    if prod.redacted_marker.is_some() {
        return 1.0;
    }

    let base = match prod.type_hint {
        ScalarTypeHint::Redacted | ScalarTypeHint::Uuid => 1.0,
        ScalarTypeHint::Integer => match prod.value.parse::<i64>() {
            Ok(n) if n >= 1000 => 0.90,
            Ok(n) if n >= 100 => 0.88,
            _ => 0.72,
        },
        ScalarTypeHint::Number => 0.55,
        ScalarTypeHint::String => {
            let v = prod.value.trim();
            let has_digit = v.bytes().any(|b| b.is_ascii_digit());
            let has_punct = v.bytes().any(|b| matches!(b, b'_' | b'-' | b':' | b'/'));
            if v.len() >= 8 && (has_digit || has_punct) {
                0.95
            } else if v.len() >= 4 && (has_digit || has_punct) {
                0.90
            } else if v.len() > 12 {
                0.78
            } else if v.len() > 6 {
                0.68
            } else {
                0.55
            }
        }
    };

    let freq = index.producer_freq.get(&prod.value).copied().unwrap_or(1);
    if freq <= 2 {
        base
    } else {
        let penalty = 0.15 * ((freq - 2).min(4) as f64);
        (base - penalty).max(0.30)
    }
}

fn rule_id_for(dep_type: &DependencyType, trans: &Transformation) -> String {
    let dt = match dep_type {
        DependencyType::ResponseToHeader => "resp_hdr",
        DependencyType::ResponseToQuery => "resp_query",
        DependencyType::ResponseToBody => "resp_body",
        DependencyType::ResponseToPath => "resp_path",
        DependencyType::SetCookieToCookie => "set_cookie",
        DependencyType::PaginationChain => "pagination",
        DependencyType::SessionChain => "session",
        DependencyType::DerivedValue => "derived",
    };
    let tr = match trans {
        Transformation::Identity => "id",
        Transformation::UrlEncoding => "url",
        Transformation::Base64 => "b64",
        Transformation::PrefixSuffix => "prefix",
        Transformation::JsonExtract => "json",
        Transformation::Custom => "custom",
    };
    format!("dep.{dt}.{tr}")
}

fn strip_auth_or_token_prefix(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    for prefix in ["bearer ", "token ", "apikey ", "api-key "] {
        if trimmed.len() > prefix.len() && trimmed[..prefix.len()].eq_ignore_ascii_case(prefix) {
            return Some(trimmed[prefix.len()..].trim());
        }
    }
    None
}

fn percent_decode(input: &str, plus_as_space: bool) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let mut changed = false;

    while i < bytes.len() {
        match bytes[i] {
            b'+' if plus_as_space => {
                out.push(b' ');
                changed = true;
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                if let (Some(hi), Some(lo)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2])) {
                    out.push((hi << 4) | lo);
                    changed = true;
                    i += 3;
                } else {
                    out.push(b'%');
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }

    if !changed {
        return None;
    }
    String::from_utf8(out).ok()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn try_decode_base64_text(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.len() < 8 || trimmed.contains(' ') {
        return None;
    }
    let bytes = STANDARD
        .decode(trimmed)
        .or_else(|_| URL_SAFE_NO_PAD.decode(trimmed))
        .or_else(|_| URL_SAFE.decode(trimmed))
        .ok()?;
    if bytes.len() < 4 {
        return None;
    }
    let text = String::from_utf8(bytes).ok()?;
    if text
        .bytes()
        .all(|b| (0x20..=0x7e).contains(&b) && b != b'\r' && b != b'\n')
    {
        Some(text)
    } else {
        None
    }
}

fn fnv1a_32(bytes: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in bytes {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

#[cfg(test)]
mod tests {
    use harflow_endpoints::discover_endpoints;
    use harflow_har::{parse_har_bytes, ImportLimits};
    use harflow_model::ConfirmationStatus;
    use harflow_normalizer::normalize_har_session;

    use super::*;

    #[test]
    fn detects_identity_prefix_url_base64_cookie_pagination_and_rejects_weak_and_reverse() {
        let raw = br#"{
          "log": {
            "version": "1.2",
            "entries": [
              {
                "startedDateTime": "2026-11-18T02:00:00Z",
                "time": 10.0,
                "request": {
                  "method": "POST",
                  "url": "https://api.example.test/oauth/token?limit=20&cursor=cur_preexisting",
                  "headers": [
                    { "name": "Content-Type", "value": "application/json" }
                  ],
                  "postData": { "mimeType": "application/json", "text": "{\"grant_type\":\"client_credentials\"}" }
                },
                "response": {
                  "status": 200,
                  "headers": [
                    { "name": "Set-Cookie", "value": "sid=hf_synthetic_sid_42; HttpOnly" }
                  ],
                  "content": {
                    "mimeType": "application/json",
                    "text": "{\"access_token\":\"hf_synthetic_tok_42\",\"limit\":20,\"ok\":true,\"note\":\"cur_preexisting\"}"
                  }
                }
              },
              {
                "startedDateTime": "2026-11-18T02:00:01Z",
                "time": 12.0,
                "request": {
                  "method": "POST",
                  "url": "https://api.example.test/v1/orders",
                  "headers": [
                    { "name": "Authorization", "value": "Bearer hf_synthetic_tok_42" },
                    { "name": "Cookie", "value": "sid=hf_synthetic_sid_42" }
                  ],
                  "postData": { "mimeType": "application/json", "text": "{\"sku\":\"sku_1\"}" }
                },
                "response": {
                  "status": 201,
                  "headers": [
                    { "name": "Location", "value": "https://api.example.test/v1/orders/ord_synthetic_1001" }
                  ],
                  "content": {
                    "mimeType": "application/json",
                    "text": "{\"id\":\"ord_synthetic_1001\",\"tagEncoded\":\"alpha/beta\",\"blobId\":\"raw_token_999\",\"reverseMarker\":\"later_only\",\"ok\":true}"
                  }
                }
              },
              {
                "startedDateTime": "2026-11-18T02:00:02Z",
                "time": 11.0,
                "request": {
                  "method": "GET",
                  "url": "https://api.example.test/v1/orders/ord_synthetic_1001?tag=alpha%2Fbeta&limit=20",
                  "headers": [
                    { "name": "Authorization", "value": "Bearer hf_synthetic_tok_42" },
                    { "name": "Cookie", "value": "sid=hf_synthetic_sid_42" },
                    { "name": "X-Blob", "value": "cmF3X3Rva2VuXzk5OQ==" }
                  ]
                },
                "response": {
                  "status": 200,
                  "content": {
                    "mimeType": "application/json",
                    "text": "{\"nextCursor\":\"cur_page_2\",\"laterUsedEarlier\":\"earlier_val\",\"ok\":true}"
                  }
                }
              },
              {
                "startedDateTime": "2026-11-18T02:00:03Z",
                "time": 9.0,
                "request": {
                  "method": "GET",
                  "url": "https://api.example.test/v1/orders/ord_synthetic_1001?tag=alpha%2Fbeta&cursor=cur_page_2&limit=20",
                  "headers": [
                    { "name": "Authorization", "value": "Bearer hf_synthetic_tok_42" },
                    { "name": "Cookie", "value": "sid=hf_synthetic_sid_42" }
                  ]
                },
                "response": {
                  "status": 200,
                  "content": { "mimeType": "application/json", "text": "{\"nextCursor\":\"cur_page_3\"}" }
                }
              }
            ]
          }
        }"#;

        let har = parse_har_bytes(raw, &ImportLimits::default()).expect("parse");
        let session = normalize_har_session("sess-0001", "eval.har", &har);
        let sessions = [session];
        let endpoints = discover_endpoints(&sessions);
        let deps = detect_dependencies(&sessions, &endpoints);

        assert!(deps.iter().any(|d| {
            d.dependency_type == DependencyType::SessionChain
                && d.transformation == Some(Transformation::PrefixSuffix)
                && d.status == ConfirmationStatus::Confirmed
                && !d.evidence.is_empty()
        }));
        assert!(deps.iter().any(|d| {
            d.dependency_type == DependencyType::SetCookieToCookie
                && d.status == ConfirmationStatus::Confirmed
        }));
        assert!(deps.iter().any(|d| {
            d.dependency_type == DependencyType::ResponseToPath
                && d.target.json_path_or_key == "order_id"
                && d.status == ConfirmationStatus::Confirmed
        }));
        assert!(deps.iter().any(|d| {
            d.dependency_type == DependencyType::PaginationChain
                && d.target.json_path_or_key == "cursor"
                && d.status == ConfirmationStatus::Confirmed
        }));
        assert!(deps
            .iter()
            .any(|d| d.transformation == Some(Transformation::UrlEncoding)));
        assert!(deps
            .iter()
            .any(|d| d.transformation == Some(Transformation::Base64)));

        // Weak limit/boolean & pre-existing client cursor must never appear.
        assert!(!deps.iter().any(|d| d.target.json_path_or_key == "limit"));
        assert!(!deps.iter().any(|d| d.source.json_path_or_key == "$.note"));
    }

    #[test]
    fn enforces_cross_session_contradiction_penalty_and_isolation() {
        // Session A returns orderId=shared_val_1 from /users -> /orders
        // Session B calls /users -> /orders WITHOUT matching shared value -> single-session observation penalized to Likely, not Confirmed,
        // and session A's values must never link into session B.
        let har_a = br#"{
          "log": { "version": "1.2", "entries": [
            { "startedDateTime": "2026-11-18T00:00:00Z", "time": 5.0,
              "request": { "method": "GET", "url": "https://api.example.test/v1/users" },
              "response": { "status": 200, "content": { "mimeType": "application/json", "text": "{\"featured\":\"item_shared_99\"}" } } },
            { "startedDateTime": "2026-11-18T00:00:01Z", "time": 5.0,
              "request": { "method": "GET", "url": "https://api.example.test/v1/orders?ref=item_shared_99" },
              "response": { "status": 200, "content": { "mimeType": "application/json", "text": "{\"ok\":true}" } } }
          ]}
        }"#;
        let har_b = br#"{
          "log": { "version": "1.2", "entries": [
            { "startedDateTime": "2026-11-18T01:00:00Z", "time": 5.0,
              "request": { "method": "GET", "url": "https://api.example.test/v1/users" },
              "response": { "status": 200, "content": { "mimeType": "application/json", "text": "{\"featured\":\"item_different\"}" } } },
            { "startedDateTime": "2026-11-18T01:00:01Z", "time": 5.0,
              "request": { "method": "GET", "url": "https://api.example.test/v1/orders?ref=other_unrelated" },
              "response": { "status": 200, "content": { "mimeType": "application/json", "text": "{\"ok\":true}" } } }
          ]}
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

        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].observed_session_ids, vec!["sess-a".to_string()]);
        assert_ne!(deps[0].status, ConfirmationStatus::Confirmed);
    }
}
