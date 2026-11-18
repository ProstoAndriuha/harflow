use std::fmt;

use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use serde_json::Value;

const SENSITIVE_HEADERS: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "cookie",
    "set-cookie",
    "x-api-key",
    "x-auth-token",
    "x-csrf-token",
    "x-xsrf-token",
    "x-amz-security-token",
    "x-goog-api-key",
    "api-key",
    "token",
    "x-session-id",
    "www-authenticate",
    "proxy-authenticate",
];

const NON_SECRET_KEY_ALLOWLIST: &[&str] = &[
    "token_type",
    "page_token",
    "next_page_token",
    "prev_page_token",
    "sync_token",
    "continuation_token",
    "grant_type",
    "expires_in",
    "scope",
];

const SENSITIVE_KEYS: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "client_secret",
    "access_token",
    "refresh_token",
    "id_token",
    "api_key",
    "apikey",
    "private_key",
    "session_token",
    "csrf_token",
    "auth_token",
    "bearer_token",
    "session_id",
];

const SENSITIVE_KEY_SUFFIXES: &[&str] = &[
    "_token",
    "_secret",
    "_password",
    "_passwd",
    "_api_key",
    "_apikey",
    "_private_key",
    "_session_id",
];

/// Wrapper around a redacted secret that guarantees the raw value cannot leak through `Debug` or `Display`.
#[derive(Clone, PartialEq, Eq)]
pub struct RedactedValue {
    kind: String,
    fingerprint: String,
}

/// Computes a deterministic 8-hex correlation fingerprint for a string without storing the raw input.
#[must_use]
pub fn fingerprint_secret(raw: &str) -> String {
    format!("{:08x}", fnv1a_64(raw.as_bytes()))
}

impl RedactedValue {
    /// Creates a deterministic redacted placeholder for correlation without retaining the secret.
    #[must_use]
    pub fn from_secret(kind: impl Into<String>, raw_secret: &str) -> Self {
        Self {
            kind: kind.into(),
            fingerprint: fingerprint_secret(raw_secret),
        }
    }

    /// Formats the canonical redacted token marker.
    #[must_use]
    pub fn marker(&self) -> String {
        format!("[REDACTED:{}:{}]", self.kind, self.fingerprint)
    }
}

impl fmt::Debug for RedactedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.marker())
    }
}

impl fmt::Display for RedactedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.marker())
    }
}

/// Returns true if an HTTP header name holds credentials or session tokens.
#[must_use]
pub fn is_sensitive_header_name(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    SENSITIVE_HEADERS.contains(&lower.as_str())
}

/// Normalizes `camelCase`, `kebab-case`, and `snake_case` key names into canonical lowercase `snake_case`.
#[must_use]
pub fn normalize_key_snake(key: &str) -> String {
    let trimmed = key.trim();
    let mut out = String::with_capacity(trimmed.len() + 4);
    let mut prev_lower_or_digit = false;
    for ch in trimmed.chars() {
        if ch == '-' || ch == '_' || ch == ' ' {
            if !out.is_empty() && !out.ends_with('_') {
                out.push('_');
            }
            prev_lower_or_digit = false;
        } else if ch.is_ascii_uppercase() {
            if prev_lower_or_digit && !out.is_empty() && !out.ends_with('_') {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            prev_lower_or_digit = false;
        } else {
            out.push(ch.to_ascii_lowercase());
            prev_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        }
    }
    out
}

/// Returns true if a JSON field, form key, or query parameter name likely holds a secret.
#[must_use]
pub fn is_sensitive_key_name(key: &str) -> bool {
    let normalized = normalize_key_snake(key);
    if NON_SECRET_KEY_ALLOWLIST.contains(&normalized.as_str()) {
        return false;
    }
    if SENSITIVE_KEYS.contains(&normalized.as_str()) {
        return true;
    }
    SENSITIVE_KEY_SUFFIXES
        .iter()
        .any(|suffix| normalized.ends_with(suffix))
}

/// Returns `true` when `s` is already a HARFlow redaction marker.
#[must_use]
pub fn is_already_redacted(s: &str) -> bool {
    parse_redacted_marker(s).is_some()
}

/// Parses a canonical HARFlow redaction marker (`[REDACTED:<kind>:<fingerprint>]`) into `(kind, fingerprint)`.
#[must_use]
pub fn parse_redacted_marker(value: &str) -> Option<(&str, &str)> {
    let inner = value.trim().strip_prefix("[REDACTED:")?.strip_suffix(']')?;
    let (kind, fingerprint) = inner.rsplit_once(':')?;
    if kind.is_empty() || fingerprint.is_empty() {
        return None;
    }
    Some((kind, fingerprint))
}

fn split_auth_scheme(raw: &str) -> Option<(&'static str, &str)> {
    let trimmed = raw.trim();
    for (prefix, scheme) in [
        ("bearer ", "bearer"),
        ("basic ", "basic"),
        ("digest ", "digest"),
    ] {
        if trimmed
            .get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
        {
            return Some((scheme, trimmed[prefix.len()..].trim()));
        }
    }
    None
}

/// Detects secret-shaped string values (`Bearer`, `Basic`, `Digest`, or 3-segment JWT)
/// and returns a deterministic redacted placeholder when matched.
#[must_use]
pub fn sanitize_secret_like_string(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || is_already_redacted(trimmed) {
        return None;
    }

    if let Some((scheme, secret)) = split_auth_scheme(trimmed) {
        if !secret.is_empty() && !secret.to_ascii_lowercase().starts_with("[redacted:") {
            let kind = match scheme {
                "bearer" => "bearer_token",
                "basic" => "basic_auth",
                "digest" => "digest_auth",
                _ => scheme,
            };
            return Some(RedactedValue::from_secret(kind, secret).marker());
        }
        return None;
    }

    if looks_like_jwt(trimmed) {
        return Some(RedactedValue::from_secret("jwt_token", trimmed).marker());
    }

    None
}

fn looks_like_jwt(candidate: &str) -> bool {
    let mut parts = candidate.split('.');
    let (Some(h), Some(p), Some(s), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };

    if h.len() < 8 || p.len() < 8 || s.len() < 8 {
        return false;
    }
    if ![h, p, s].iter().all(|seg| {
        seg.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    }) {
        return false;
    }

    let decoded = URL_SAFE_NO_PAD
        .decode(h)
        .or_else(|_| URL_SAFE.decode(h))
        .ok();
    match decoded.and_then(|bytes| String::from_utf8(bytes).ok()) {
        Some(text) => text.trim_start().starts_with('{'),
        None => false,
    }
}

/// Redacts all cookie values inside a `Cookie` header while preserving cookie names and order.
#[must_use]
pub fn sanitize_cookie_header(raw: &str) -> (String, Vec<(String, String)>, usize) {
    let mut formatted_parts = Vec::new();
    let mut pairs = Vec::new();
    let mut count = 0usize;

    for segment in raw.split(';') {
        let trimmed = segment.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((name, val)) = trimmed.split_once('=') {
            let clean_name = name.trim().to_string();
            let clean_val = val.trim();
            let redacted = if is_already_redacted(clean_val) {
                clean_val.to_string()
            } else {
                count += 1;
                RedactedValue::from_secret("cookie", clean_val).marker()
            };
            formatted_parts.push(format!("{clean_name}={redacted}"));
            pairs.push((clean_name, redacted));
        } else {
            let redacted = if is_already_redacted(trimmed) {
                trimmed.to_string()
            } else {
                count += 1;
                RedactedValue::from_secret("cookie", trimmed).marker()
            };
            formatted_parts.push(redacted);
        }
    }

    (formatted_parts.join("; "), pairs, count)
}

/// Redacts the cookie value inside a `Set-Cookie` header using the `"cookie"` kind so
/// `Set-Cookie` and subsequent `Cookie` headers share identical fingerprints, while preserving attributes.
#[must_use]
pub fn sanitize_set_cookie_header(raw: &str) -> (String, Option<(String, String)>, usize) {
    let (first, attrs) = match raw.split_once(';') {
        Some((f, r)) => (f.trim(), Some(r.trim())),
        None => (raw.trim(), None),
    };

    if let Some((name, val)) = first.split_once('=') {
        let clean_name = name.trim().to_string();
        let clean_val = val.trim();
        let (redacted, count) = if is_already_redacted(clean_val) {
            (clean_val.to_string(), 0)
        } else {
            (RedactedValue::from_secret("cookie", clean_val).marker(), 1)
        };
        let rebuilt = match attrs {
            Some(rest) if !rest.is_empty() => format!("{clean_name}={redacted}; {rest}"),
            _ => format!("{clean_name}={redacted}"),
        };
        (rebuilt, Some((clean_name, redacted)), count)
    } else if is_already_redacted(raw) {
        (raw.to_string(), None, 0)
    } else {
        (RedactedValue::from_secret("cookie", raw).marker(), None, 1)
    }
}

/// Sanitizes an `application/x-www-form-urlencoded` body while preserving field order and duplicates.
#[must_use]
pub fn sanitize_form_urlencoded(raw: &str) -> (String, Vec<(String, String)>, usize) {
    let mut pairs = Vec::new();
    let mut serialized = Vec::new();
    let mut count = 0usize;

    for pair in raw.split('&').filter(|p| !p.is_empty()) {
        let (raw_key, raw_val) = match pair.split_once('=') {
            Some((k, v)) => (k, v),
            None => (pair, ""),
        };
        let decoded_key = percent_decode_form(raw_key);
        let decoded_val = percent_decode_form(raw_val);

        let clean_val = if is_already_redacted(&decoded_val) {
            decoded_val
        } else if is_sensitive_key_name(&decoded_key) {
            count += 1;
            RedactedValue::from_secret(normalize_key_snake(&decoded_key), &decoded_val).marker()
        } else if let Some(secret_redacted) = sanitize_secret_like_string(&decoded_val) {
            count += 1;
            secret_redacted
        } else {
            decoded_val
        };

        serialized.push(if raw_val.is_empty() && !pair.contains('=') {
            raw_key.to_string()
        } else {
            format!("{raw_key}={clean_val}")
        });
        pairs.push((decoded_key, clean_val));
    }

    (serialized.join("&"), pairs, count)
}

fn percent_decode_form(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                if let (Some(hi), Some(lo)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2])) {
                    out.push((hi << 4) | lo);
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
    String::from_utf8_lossy(&out).into_owned()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Redacts a header value when the header name is sensitive or when the value matches a secret pattern.
#[must_use]
pub fn sanitize_header_value(name: &str, value: &str) -> (String, bool) {
    let lower_name = name.trim().to_ascii_lowercase();
    if lower_name == "cookie" {
        let (clean, _, count) = sanitize_cookie_header(value);
        return (clean, count > 0);
    }
    if lower_name == "set-cookie" {
        let (clean, _, count) = sanitize_set_cookie_header(value);
        return (clean, count > 0);
    }

    if is_sensitive_header_name(name) {
        if is_already_redacted(value) {
            return (value.to_string(), false);
        }
        let base_kind = lower_name.replace('-', "_");
        if matches!(lower_name.as_str(), "authorization" | "proxy-authorization") {
            if let Some((scheme, secret)) = split_auth_scheme(value) {
                if secret.to_ascii_lowercase().starts_with("[redacted:") {
                    return (value.to_string(), false);
                }
                if !secret.is_empty() {
                    return (
                        RedactedValue::from_secret(format!("{base_kind}_{scheme}"), secret)
                            .marker(),
                        true,
                    );
                }
            }
        }
        return (RedactedValue::from_secret(base_kind, value).marker(), true);
    }

    if let Some(redacted) = sanitize_secret_like_string(value) {
        return (redacted, true);
    }

    (value.to_string(), false)
}

/// Redacts a query parameter value when its key is sensitive or its value looks like a secret token.
#[must_use]
pub fn sanitize_query_param(name: &str, value: &str) -> (String, bool) {
    if is_already_redacted(value) {
        return (value.to_string(), false);
    }
    if is_sensitive_key_name(name) {
        let kind = normalize_key_snake(name);
        return (RedactedValue::from_secret(kind, value).marker(), true);
    }
    if let Some(redacted) = sanitize_secret_like_string(value) {
        return (redacted, true);
    }
    (value.to_string(), false)
}

/// Recursively sanitizes a JSON tree, returning the clean JSON value and redaction count.
#[must_use]
pub fn sanitize_json_value(input: &Value) -> (Value, usize) {
    match input {
        Value::Object(map) => {
            let mut count = 0usize;
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, v) in map {
                if is_sensitive_key_name(k) {
                    if v.as_str().is_some_and(is_already_redacted) {
                        out.insert(k.clone(), v.clone());
                    } else {
                        let raw = v
                            .as_str()
                            .map_or_else(|| v.to_string(), ToString::to_string);
                        let marker =
                            RedactedValue::from_secret(normalize_key_snake(k), &raw).marker();
                        out.insert(k.clone(), Value::String(marker));
                        count += 1;
                    }
                } else {
                    let (clean_child, child_count) = sanitize_json_value(v);
                    count += child_count;
                    out.insert(k.clone(), clean_child);
                }
            }
            (Value::Object(out), count)
        }
        Value::Array(items) => {
            let mut count = 0usize;
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                let (clean_item, item_count) = sanitize_json_value(item);
                count += item_count;
                out.push(clean_item);
            }
            (Value::Array(out), count)
        }
        Value::String(s) => {
            if let Some(redacted) = sanitize_secret_like_string(s) {
                (Value::String(redacted), 1)
            } else {
                (Value::String(s.clone()), 0)
            }
        }
        _ => (input.clone(), 0),
    }
}

/// Checks text exports (SDKs, reports) for obvious unredacted secret header/token leaks.
#[must_use]
pub fn contains_unredacted_secret_marker(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    for line in lower.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("authorization:") {
            if !rest.contains("[redacted:") {
                return true;
            }
        }
        if let Some(rest) = trimmed.strip_prefix("bearer ") {
            if !rest.starts_with("[redacted:") && rest.len() > 8 {
                return true;
            }
        }
    }
    false
}

fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash & 0xffff_ffff
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_synthetic_jwt() -> String {
        let hdr = URL_SAFE_NO_PAD.encode(br#"{"alg":"HS256","typ":"JWT"}"#);
        let payload = URL_SAFE_NO_PAD.encode(br#"{"sub":"hf_user_01","scope":"read"}"#);
        format!("{hdr}.{payload}.syntheticSig12345")
    }

    #[test]
    fn redacted_value_never_exposes_raw_secret_in_debug_or_display() {
        let secret = "hf_synthetic_super_secret_value_12345";
        let redacted = RedactedValue::from_secret("authorization", secret);
        let debug_str = format!("{redacted:?}");
        let display_str = format!("{redacted}");
        assert!(!debug_str.contains(secret));
        assert!(!display_str.contains(secret));
        assert!(display_str.starts_with("[REDACTED:authorization:"));
    }

    #[test]
    fn sanitizes_headers_and_nested_json_deterministically() {
        let (clean_auth, changed) =
            sanitize_header_value("Authorization", "Bearer hf_synthetic_abc");
        assert!(changed);
        assert!(!clean_auth.contains("hf_synthetic_abc"));

        let payload = serde_json::json!({
            "user": "alice",
            "access_token": "hf_synthetic_access_999",
            "nested": { "password": "hf_synthetic_password" }
        });
        let (sanitized, count) = sanitize_json_value(&payload);
        assert_eq!(count, 2);
        let serialized = sanitized.to_string();
        assert!(!serialized.contains("hf_synthetic_access_999"));
        assert!(!serialized.contains("hf_synthetic_password"));
        assert!(serialized.contains("alice"));
    }

    #[test]
    fn detects_jwt_basic_and_preserves_pagination_tokens() {
        let jwt = make_synthetic_jwt();
        let (clean_query, q_changed) = sanitize_query_param("state_blob", &jwt);
        assert!(q_changed);
        assert!(!clean_query.contains(&jwt));
        assert!(clean_query.starts_with("[REDACTED:jwt_token:"));

        let json_with_jwt = serde_json::json!({
            "data": jwt,
            "page_token": "cursor_page_2",
            "next_page_token": "cursor_page_3"
        });
        let (clean_json, count) = sanitize_json_value(&json_with_jwt);
        assert_eq!(count, 1);
        assert_eq!(clean_json["page_token"], "cursor_page_2");
        assert_eq!(clean_json["next_page_token"], "cursor_page_3");

        let (reclean, recount) = sanitize_json_value(&clean_json);
        assert_eq!(reclean, clean_json);
        assert_eq!(recount, 0);

        let (basic, b_changed) = sanitize_header_value("X-Proxy-Auth", "Basic dXNlcjpwYXNz");
        assert!(b_changed);
        assert!(basic.starts_with("[REDACTED:basic_auth:"));
    }

    #[test]
    fn correlates_set_cookie_and_cookie_fingerprints_and_redacts_form_bodies() {
        let (set_cookie_hdr, set_pair, sc_count) =
            sanitize_set_cookie_header("sid=hf_synthetic_sess_77; HttpOnly; Secure");
        let (cookie_hdr, cookie_pairs, c_count) =
            sanitize_cookie_header("sid=hf_synthetic_sess_77; theme=dark");

        assert_eq!(sc_count, 1);
        assert_eq!(c_count, 2);
        assert!(set_cookie_hdr.contains("; HttpOnly; Secure"));
        assert!(!set_cookie_hdr.contains("hf_synthetic_sess_77"));
        assert!(!cookie_hdr.contains("hf_synthetic_sess_77"));
        assert_eq!(set_pair.expect("pair").1, cookie_pairs[0].1);

        let form = "grant_type=password&password=hf_synthetic_pw&client_secret=hf_synthetic_cs&tag=a&tag=b";
        let (clean_form, pairs, form_count) = sanitize_form_urlencoded(form);
        assert_eq!(form_count, 2);
        assert!(!clean_form.contains("hf_synthetic_pw"));
        assert!(!clean_form.contains("hf_synthetic_cs"));
        assert_eq!(pairs[0], ("grant_type".into(), "password".into()));
        assert_eq!(pairs[3], ("tag".into(), "a".into()));
        assert_eq!(pairs[4], ("tag".into(), "b".into()));
    }

    #[test]
    fn correlates_json_access_token_and_bearer_header_fingerprints() {
        let token = "hf_synthetic_Tok_42";
        let (clean_json, j_count) =
            sanitize_json_value(&serde_json::json!({ "access_token": token }));
        assert_eq!(j_count, 1);
        let json_marker = clean_json["access_token"].as_str().expect("json marker");

        let (clean_hdr, h_changed) =
            sanitize_header_value("Authorization", &format!("Bearer {token}"));
        assert!(h_changed);
        assert!(!clean_hdr.contains(token));

        let (j_kind, j_fp) = parse_redacted_marker(json_marker).expect("parse json marker");
        let (h_kind, h_fp) = parse_redacted_marker(&clean_hdr).expect("parse hdr marker");
        assert_eq!(j_kind, "access_token");
        assert_eq!(h_kind, "authorization_bearer");
        assert_eq!(j_fp, h_fp);
        assert!(parse_redacted_marker("plain-value").is_none());
    }

    #[test]
    fn handles_multibyte_utf8_strings_without_panicking() {
        let utf8_text = "ほっぺの　りょうがわに\nちいさい　でんきぶくろを　もつ。";
        assert!(sanitize_secret_like_string(utf8_text).is_none());
        let (val, count) = sanitize_json_value(&serde_json::json!({ "flavor_text": utf8_text }));
        assert_eq!(count, 0);
        assert_eq!(val["flavor_text"], utf8_text);
    }

    #[test]
    fn redacts_camelcase_keys_and_preserves_camelcase_pagination_tokens() {
        let payload = serde_json::json!({
            "accessToken": "hf_synthetic_opaque_tok",
            "refreshToken": "hf_synthetic_refresh_tok",
            "nextPageToken": "cursor_2",
            "tokenType": "Bearer"
        });
        let (clean, count) = sanitize_json_value(&payload);
        assert_eq!(count, 2);
        let access_marker = clean["accessToken"].as_str().expect("redacted accessToken");
        assert!(access_marker.starts_with("[REDACTED:access_token:"));
        assert!(clean["refreshToken"]
            .as_str()
            .expect("redacted refreshToken")
            .starts_with("[REDACTED:refresh_token:"));
        assert_eq!(clean["nextPageToken"], "cursor_2");
        assert_eq!(clean["tokenType"], "Bearer");
    }
}
