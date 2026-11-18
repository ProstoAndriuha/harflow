use harflow_core::HarflowError;
use serde::{Deserialize, Serialize};

/// Configurable safety boundaries for HAR ingestion per SRS M02.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportLimits {
    pub default_max_bytes: usize,
    pub hard_max_bytes: usize,
    pub max_entries: usize,
    pub max_json_depth: usize,
}

impl Default for ImportLimits {
    fn default() -> Self {
        Self {
            default_max_bytes: 100 * 1024 * 1024,
            hard_max_bytes: 500 * 1024 * 1024,
            max_entries: 100_000,
            max_json_depth: 64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarFile {
    pub log: HarLog,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarLog {
    pub version: String,
    #[serde(default)]
    pub creator: Option<HarCreator>,
    #[serde(default)]
    pub entries: Vec<HarEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarCreator {
    pub name: String,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarEntry {
    #[serde(default)]
    pub started_date_time: String,
    #[serde(default)]
    pub time: f64,
    pub request: HarRequest,
    pub response: HarResponse,
    #[serde(default)]
    pub pageref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarRequest {
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub http_version: Option<String>,
    #[serde(default)]
    pub headers: Vec<HarNameValue>,
    #[serde(default)]
    pub query_string: Vec<HarNameValue>,
    #[serde(default)]
    pub cookies: Vec<HarCookie>,
    #[serde(default)]
    pub post_data: Option<HarPostData>,
    #[serde(default)]
    pub body_size: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarResponse {
    pub status: u16,
    #[serde(default)]
    pub status_text: String,
    #[serde(default)]
    pub headers: Vec<HarNameValue>,
    #[serde(default)]
    pub cookies: Vec<HarCookie>,
    #[serde(default)]
    pub content: HarContent,
    #[serde(default)]
    pub body_size: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarNameValue {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarCookie {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub http_only: Option<bool>,
    #[serde(default)]
    pub secure: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarPostData {
    #[serde(default)]
    pub mime_type: String,
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HarContent {
    #[serde(default)]
    pub size: Option<i64>,
    #[serde(default)]
    pub mime_type: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub encoding: Option<String>,
}

/// Validates input bounds, JSON nesting depth, and HAR 1.2 structure before parsing.
pub fn parse_har_bytes(bytes: &[u8], limits: &ImportLimits) -> Result<HarFile, HarflowError> {
    let effective_limit = limits.default_max_bytes.min(limits.hard_max_bytes);
    if bytes.len() > effective_limit {
        return Err(HarflowError::InvalidSource(format!(
            "HAR input exceeds size limit ({} > {} bytes)",
            bytes.len(),
            effective_limit
        )));
    }

    if bytes.is_empty() {
        return Err(HarflowError::InvalidSource(
            "HAR input is empty".to_string(),
        ));
    }

    check_json_nesting_depth(bytes, limits.max_json_depth)?;

    let har: HarFile = serde_json::from_slice(bytes)
        .map_err(|err| HarflowError::InvalidSource(format!("malformed HAR JSON: {err}")))?;

    if har.log.version.trim().is_empty() {
        return Err(HarflowError::InvalidSource(
            "HAR log.version must not be empty".to_string(),
        ));
    }

    if har.log.entries.len() > limits.max_entries {
        return Err(HarflowError::InvalidSource(format!(
            "HAR entry count ({}) exceeds limit ({})",
            har.log.entries.len(),
            limits.max_entries
        )));
    }

    Ok(har)
}

fn check_json_nesting_depth(bytes: &[u8], max_depth: usize) -> Result<(), HarflowError> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for &b in bytes {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }

        match b {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth += 1;
                if depth > max_depth {
                    return Err(HarflowError::InvalidSource(format!(
                        "JSON nesting depth exceeds limit ({max_depth})"
                    )));
                }
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_valid_har() {
        let raw = br#"{
          "log": {
            "version": "1.2",
            "creator": { "name": "test", "version": "1.0" },
            "entries": []
          }
        }"#;
        let parsed = parse_har_bytes(raw, &ImportLimits::default()).expect("parse valid har");
        assert_eq!(parsed.log.version, "1.2");
    }

    #[test]
    fn rejects_oversized_and_deeply_nested_inputs() {
        let limits = ImportLimits {
            default_max_bytes: 16,
            hard_max_bytes: 32,
            max_entries: 10,
            max_json_depth: 4,
        };
        let oversized = vec![b' '; 64];
        let err = parse_har_bytes(&oversized, &limits).unwrap_err();
        assert_eq!(err.exit_code().as_i32(), 3);

        let nested = b"[[[[[]]]]]";
        let err_depth = parse_har_bytes(nested, &limits).unwrap_err();
        assert_eq!(err_depth.exit_code().as_i32(), 3);
    }

    #[test]
    fn rejects_missing_log_or_malformed_json() {
        let malformed = br#"{"not_log": {}}"#;
        let err = parse_har_bytes(malformed, &ImportLimits::default()).unwrap_err();
        assert_eq!(err.exit_code().as_i32(), 3);
    }
}
