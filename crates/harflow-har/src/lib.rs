use std::fmt;
use std::io::{self, BufRead, BufReader, Read};

use flate2::read::GzDecoder;
use harflow_core::HarflowError;
use serde::de::{self, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Configurable safety boundaries for HAR ingestion per SRS M02.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportLimits {
    pub default_max_bytes: usize,
    pub hard_max_bytes: usize,
    pub max_entries: usize,
    pub max_json_depth: usize,
    pub tolerate_partial_entries: bool,
}

impl Default for ImportLimits {
    fn default() -> Self {
        Self {
            default_max_bytes: 100 * 1024 * 1024,
            hard_max_bytes: 500 * 1024 * 1024,
            max_entries: 100_000,
            max_json_depth: 64,
            tolerate_partial_entries: true,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HarInitiator {
    #[serde(rename = "type", default)]
    pub initiator_type: String,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarEntry {
    #[serde(default, deserialize_with = "deserialize_string_or_null")]
    pub started_date_time: String,
    #[serde(default, deserialize_with = "deserialize_f64_or_null")]
    pub time: f64,
    pub request: HarRequest,
    #[serde(default)]
    pub response: HarResponse,
    #[serde(default)]
    pub pageref: Option<String>,
    #[serde(rename = "_initiator", default)]
    pub initiator: Option<HarInitiator>,
}

fn deserialize_string_or_null<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

fn deserialize_f64_or_null<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<f64>::deserialize(deserializer)?.unwrap_or(0.0))
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HarResponse {
    #[serde(default)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedEntryWarning {
    pub index: usize,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarStreamSummary {
    pub version: String,
    pub creator: Option<HarCreator>,
    pub processed_entries: usize,
    pub skipped_entries: Vec<SkippedEntryWarning>,
}

/// Parses HAR 1.2 bytes (plain JSON or `.har.gz`) with strict size and JSON nesting limits.
pub fn parse_har_bytes(bytes: &[u8], limits: &ImportLimits) -> Result<HarFile, HarflowError> {
    let mut entries = Vec::new();
    let summary = stream_har_reader(bytes, limits, |_idx, entry| {
        entries.push(entry);
        Ok(())
    })?;

    Ok(HarFile {
        log: HarLog {
            version: summary.version,
            creator: summary.creator,
            entries,
        },
    })
}

/// Streams HAR 1.2 entries from any `Read` source (plain JSON or gzip) with bounded memory,
/// decompression-bomb protection, nesting-depth enforcement, and partial-entry tolerance.
pub fn stream_har_reader<R, F>(
    reader: R,
    limits: &ImportLimits,
    mut on_entry: F,
) -> Result<HarStreamSummary, HarflowError>
where
    R: Read,
    F: FnMut(usize, HarEntry) -> Result<(), HarflowError>,
{
    let effective_limit = limits.default_max_bytes.min(limits.hard_max_bytes);
    if effective_limit == 0 {
        return Err(HarflowError::InvalidSource(
            "HAR effective size limit must be greater than zero".to_string(),
        ));
    }

    let mut peek_buf = BufReader::new(reader);
    let head = peek_buf
        .fill_buf()
        .map_err(|err| HarflowError::InvalidSource(format!("cannot read HAR input: {err}")))?;

    if head.is_empty() {
        return Err(HarflowError::InvalidSource(
            "HAR input is empty".to_string(),
        ));
    }

    let is_gzip = head.len() >= 2 && head[0] == 0x1f && head[1] == 0x8b;

    if is_gzip {
        let compressed_bounded = BoundedReader::new(peek_buf, effective_limit, None);
        let decoder = GzDecoder::new(compressed_bounded);
        let decompressed_bounded =
            BoundedReader::new(decoder, effective_limit, Some(limits.max_json_depth));
        parse_stream(decompressed_bounded, limits, &mut on_entry)
    } else {
        let plain_bounded =
            BoundedReader::new(peek_buf, effective_limit, Some(limits.max_json_depth));
        parse_stream(plain_bounded, limits, &mut on_entry)
    }
}

fn parse_stream<R, F>(
    reader: R,
    limits: &ImportLimits,
    on_entry: &mut F,
) -> Result<HarStreamSummary, HarflowError>
where
    R: Read,
    F: FnMut(usize, HarEntry) -> Result<(), HarflowError>,
{
    let buffered = BufReader::with_capacity(64 * 1024, reader);
    let mut deserializer = serde_json::Deserializer::from_reader(buffered);

    let mut state = StreamState {
        limits,
        on_entry,
        version: None,
        creator: None,
        processed_entries: 0,
        skipped_entries: Vec::new(),
        callback_error: None,
    };

    let seed = HarRootSeed { state: &mut state };
    let parse_res = serde::de::DeserializeSeed::deserialize(seed, &mut deserializer);

    if let Some(cb_err) = state.callback_error.take() {
        return Err(cb_err);
    }

    let saw_log = parse_res
        .map_err(|err| HarflowError::InvalidSource(format!("malformed HAR JSON: {err}")))?;

    deserializer
        .end()
        .map_err(|err| HarflowError::InvalidSource(format!("trailing HAR JSON content: {err}")))?;

    if !saw_log {
        return Err(HarflowError::InvalidSource(
            "HAR root object is missing required 'log' field".to_string(),
        ));
    }

    let version = state
        .version
        .ok_or_else(|| HarflowError::InvalidSource("HAR log.version is missing".to_string()))?;

    if version.trim().is_empty() {
        return Err(HarflowError::InvalidSource(
            "HAR log.version must not be empty".to_string(),
        ));
    }

    Ok(HarStreamSummary {
        version,
        creator: state.creator,
        processed_entries: state.processed_entries,
        skipped_entries: state.skipped_entries,
    })
}

struct BoundedReader<R> {
    inner: R,
    bytes_read: usize,
    max_bytes: usize,
    max_json_depth: Option<usize>,
    depth: usize,
    in_string: bool,
    escaped: bool,
}

impl<R> BoundedReader<R> {
    fn new(inner: R, max_bytes: usize, max_json_depth: Option<usize>) -> Self {
        Self {
            inner,
            bytes_read: 0,
            max_bytes,
            max_json_depth,
            depth: 0,
            in_string: false,
            escaped: false,
        }
    }
}

impl<R: Read> Read for BoundedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        if n == 0 {
            return Ok(0);
        }

        self.bytes_read = self.bytes_read.saturating_add(n);
        if self.bytes_read > self.max_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "HAR input exceeds size limit ({} > {} bytes)",
                    self.bytes_read, self.max_bytes
                ),
            ));
        }

        if let Some(max_depth) = self.max_json_depth {
            for &b in &buf[..n] {
                if self.in_string {
                    if self.escaped {
                        self.escaped = false;
                    } else if b == b'\\' {
                        self.escaped = true;
                    } else if b == b'"' {
                        self.in_string = false;
                    }
                    continue;
                }

                match b {
                    b'"' => self.in_string = true,
                    b'{' | b'[' => {
                        self.depth += 1;
                        if self.depth > max_depth {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!("JSON nesting depth exceeds limit ({max_depth})"),
                            ));
                        }
                    }
                    b'}' | b']' => {
                        self.depth = self.depth.saturating_sub(1);
                    }
                    _ => {}
                }
            }
        }

        Ok(n)
    }
}

struct StreamState<'a, F> {
    limits: &'a ImportLimits,
    on_entry: &'a mut F,
    version: Option<String>,
    creator: Option<HarCreator>,
    processed_entries: usize,
    skipped_entries: Vec<SkippedEntryWarning>,
    callback_error: Option<HarflowError>,
}

struct HarRootSeed<'a, 'b, F> {
    state: &'a mut StreamState<'b, F>,
}

impl<'de, 'a, 'b, F> de::DeserializeSeed<'de> for HarRootSeed<'a, 'b, F>
where
    F: FnMut(usize, HarEntry) -> Result<(), HarflowError>,
{
    type Value = bool;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(self)
    }
}

impl<'de, 'a, 'b, F> Visitor<'de> for HarRootSeed<'a, 'b, F>
where
    F: FnMut(usize, HarEntry) -> Result<(), HarflowError>,
{
    type Value = bool;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a HAR 1.2 root object containing 'log'")
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut saw_log = false;
        while let Some(key) = map.next_key::<String>()? {
            if key == "log" {
                if saw_log {
                    return Err(de::Error::duplicate_field("log"));
                }
                map.next_value_seed(HarLogSeed { state: self.state })?;
                saw_log = true;
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(saw_log)
    }
}

struct HarLogSeed<'a, 'b, F> {
    state: &'a mut StreamState<'b, F>,
}

impl<'de, 'a, 'b, F> de::DeserializeSeed<'de> for HarLogSeed<'a, 'b, F>
where
    F: FnMut(usize, HarEntry) -> Result<(), HarflowError>,
{
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(self)
    }
}

impl<'de, 'a, 'b, F> Visitor<'de> for HarLogSeed<'a, 'b, F>
where
    F: FnMut(usize, HarEntry) -> Result<(), HarflowError>,
{
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a HAR log object")
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "version" => {
                    self.state.version = Some(map.next_value::<String>()?);
                }
                "creator" => {
                    self.state.creator = map.next_value::<Option<HarCreator>>()?;
                }
                "entries" => {
                    map.next_value_seed(EntriesSeqSeed { state: self.state })?;
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(())
    }
}

struct EntriesSeqSeed<'a, 'b, F> {
    state: &'a mut StreamState<'b, F>,
}

impl<'de, 'a, 'b, F> de::DeserializeSeed<'de> for EntriesSeqSeed<'a, 'b, F>
where
    F: FnMut(usize, HarEntry) -> Result<(), HarflowError>,
{
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(self)
    }
}

impl<'de, 'a, 'b, F> Visitor<'de> for EntriesSeqSeed<'a, 'b, F>
where
    F: FnMut(usize, HarEntry) -> Result<(), HarflowError>,
{
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a HAR entries array")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut index = 0usize;
        while let Some(raw_val) = seq.next_element::<Value>()? {
            let total_seen = index + 1;
            if total_seen > self.state.limits.max_entries {
                return Err(de::Error::custom(format!(
                    "HAR entry count ({total_seen}) exceeds limit ({})",
                    self.state.limits.max_entries
                )));
            }

            match parse_and_validate_entry(raw_val) {
                Ok(entry) => {
                    if let Err(err) = (self.state.on_entry)(index, entry) {
                        self.state.callback_error = Some(err);
                        return Err(de::Error::custom("entry consumer failed"));
                    }
                    self.state.processed_entries += 1;
                }
                Err(reason) => {
                    if self.state.limits.tolerate_partial_entries {
                        self.state
                            .skipped_entries
                            .push(SkippedEntryWarning { index, reason });
                    } else {
                        return Err(de::Error::custom(format!(
                            "invalid HAR entry at index {index}: {reason}"
                        )));
                    }
                }
            }

            index += 1;
        }
        Ok(())
    }
}

fn parse_and_validate_entry(raw_val: Value) -> Result<HarEntry, String> {
    let entry: HarEntry =
        serde_json::from_value(raw_val).map_err(|err| format!("malformed entry JSON: {err}"))?;
    if entry.request.method.trim().is_empty() {
        return Err("missing request.method".to_string());
    }
    if entry.request.url.trim().is_empty() {
        return Err("missing request.url".to_string());
    }
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::write::GzEncoder;
    use flate2::Compression;

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
    fn streams_entries_even_when_entries_precedes_version_and_recovers_partial_entries() {
        let raw = br#"{
          "log": {
            "entries": [
              {
                "request": { "method": "GET", "url": "https://api.example.test/v1/ok" },
                "response": { "status": 200 }
              },
              {
                "request": { "method": "", "url": "https://api.example.test/broken" },
                "response": { "status": 500 }
              },
              {
                "not_a_request": true
              },
              {
                "_initiator": { "type": "script", "url": "https://app.example.test/main.js" },
                "request": { "method": "POST", "url": "https://api.example.test/v1/items" }
              }
            ],
            "version": "1.2"
          }
        }"#;

        let mut collected = Vec::new();
        let summary = stream_har_reader(&raw[..], &ImportLimits::default(), |_idx, entry| {
            collected.push(entry);
            Ok(())
        })
        .expect("stream valid and partial entries");

        assert_eq!(summary.version, "1.2");
        assert_eq!(summary.processed_entries, 2);
        assert_eq!(summary.skipped_entries.len(), 2);
        assert_eq!(collected[0].request.method, "GET");
        assert_eq!(
            collected[1]
                .initiator
                .as_ref()
                .map(|i| i.initiator_type.as_str()),
            Some("script")
        );
    }

    #[test]
    fn parses_gzipped_har_and_blocks_decompression_bombs() {
        let raw = br#"{
          "log": {
            "version": "1.2",
            "entries": [
              {
                "request": { "method": "GET", "url": "https://api.example.test/v1/gz" },
                "response": { "status": 200 }
              }
            ]
          }
        }"#;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(raw).expect("write gz");
        let gz_bytes = encoder.finish().expect("finish gz");

        let parsed = parse_har_bytes(&gz_bytes, &ImportLimits::default()).expect("parse gz har");
        assert_eq!(parsed.log.entries.len(), 1);
        assert_eq!(
            parsed.log.entries[0].request.url,
            "https://api.example.test/v1/gz"
        );

        // Create a tiny gzip payload (~150 bytes) that expands to 64 KB of whitespace/brackets.
        let bomb_plain = vec![b' '; 64 * 1024];
        let mut bomb_enc = GzEncoder::new(Vec::new(), Compression::best());
        bomb_enc.write_all(&bomb_plain).expect("write bomb");
        let bomb_gz = bomb_enc.finish().expect("finish bomb");

        let tight_limits = ImportLimits {
            default_max_bytes: 4096,
            hard_max_bytes: 4096,
            ..ImportLimits::default()
        };
        let err = parse_har_bytes(&bomb_gz, &tight_limits).unwrap_err();
        assert_eq!(err.exit_code().as_i32(), 3);
    }

    #[test]
    fn rejects_oversized_and_deeply_nested_inputs() {
        let limits = ImportLimits {
            default_max_bytes: 16,
            hard_max_bytes: 32,
            max_entries: 10,
            max_json_depth: 4,
            tolerate_partial_entries: true,
        };
        let oversized = vec![b' '; 64];
        let err = parse_har_bytes(&oversized, &limits).unwrap_err();
        assert_eq!(err.exit_code().as_i32(), 3);

        let nested = b"[[[[[]]]]]";
        let err_depth = parse_har_bytes(nested, &limits).unwrap_err();
        assert_eq!(err_depth.exit_code().as_i32(), 3);
    }

    #[test]
    fn rejects_missing_log_or_truncated_json() {
        let malformed = br#"{"not_log": {}}"#;
        let err = parse_har_bytes(malformed, &ImportLimits::default()).unwrap_err();
        assert_eq!(err.exit_code().as_i32(), 3);

        let truncated = br#"{"log":{"version":"1.2","entries":[{"request":{"method":"GET""#;
        let err_trunc = parse_har_bytes(truncated, &ImportLimits::default()).unwrap_err();
        assert_eq!(err_trunc.exit_code().as_i32(), 3);
    }
}
