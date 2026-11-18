use std::fs;
use std::path::PathBuf;

use harflow_har::{parse_har_bytes, ImportLimits};
use harflow_normalizer::normalize_har_session;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates dir")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

#[test]
fn redaction_sample_fixture_strips_all_raw_synthetic_secrets() {
    let fixture = repo_root().join("fixtures/synthetic-secrets/redaction-sample.har.json");
    let raw_bytes = fs::read(&fixture).expect("read redaction-sample.har.json");
    let har = parse_har_bytes(&raw_bytes, &ImportLimits::default()).expect("parse fixture");
    let normalized = normalize_har_session("sec-test", "redaction-sample.har.json", &har);

    let serialized = serde_json::to_string(&normalized).expect("serialize normalized session");
    for secret in [
        "hf_synthetic_bearer_secret_0001",
        "hf_synthetic_cookie_secret_0002",
        "hf_synthetic_password_0003",
        "hf_synthetic_api_key_0004",
    ] {
        assert!(
            !serialized.contains(secret),
            "raw secret '{secret}' leaked into normalized session JSON"
        );
    }
    assert!(normalized.redacted_secrets_count >= 4);
}
