use std::fmt;

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
];

/// Wrapper around a redacted secret that guarantees the raw value cannot leak through `Debug` or `Display`.
#[derive(Clone, PartialEq, Eq)]
pub struct RedactedValue {
    kind: String,
    fingerprint: String,
}

impl RedactedValue {
    /// Creates a deterministic redacted placeholder for correlation without retaining the secret.
    #[must_use]
    pub fn from_secret(kind: impl Into<String>, raw_secret: &str) -> Self {
        let kind_str = kind.into();
        let hash = fnv1a_64(raw_secret.as_bytes());
        Self {
            kind: kind_str,
            fingerprint: format!("{hash:08x}"),
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

/// Returns true if a JSON field or query parameter name likely holds a secret.
#[must_use]
pub fn is_sensitive_key_name(key: &str) -> bool {
    let lower = key.trim().to_ascii_lowercase();
    SENSITIVE_KEYS.contains(&lower.as_str())
}

/// Redacts a header value when the header name is sensitive or when the value resembles a Bearer token.
#[must_use]
pub fn sanitize_header_value(name: &str, value: &str) -> (String, bool) {
    if is_sensitive_header_name(name) {
        let kind = name.trim().to_ascii_lowercase().replace('-', "_");
        return (RedactedValue::from_secret(kind, value).marker(), true);
    }

    if value
        .trim_start()
        .to_ascii_lowercase()
        .starts_with("bearer ")
    {
        return (
            RedactedValue::from_secret("bearer_token", value).marker(),
            true,
        );
    }

    (value.to_string(), false)
}

/// Redacts a query parameter value when its key is sensitive.
#[must_use]
pub fn sanitize_query_param(name: &str, value: &str) -> (String, bool) {
    if is_sensitive_key_name(name) {
        let kind = name.trim().to_ascii_lowercase();
        return (RedactedValue::from_secret(kind, value).marker(), true);
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
                    let raw = v
                        .as_str()
                        .map_or_else(|| v.to_string(), ToString::to_string);
                    let marker = RedactedValue::from_secret(k.to_ascii_lowercase(), &raw).marker();
                    out.insert(k.clone(), Value::String(marker));
                    count += 1;
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
            if s.trim_start().to_ascii_lowercase().starts_with("bearer ") {
                let marker = RedactedValue::from_secret("bearer_token", s).marker();
                (Value::String(marker), 1)
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
}
