use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use flate2::write::GzEncoder;
use flate2::Compression;
use uuid::Uuid;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates dir")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn bin_path() -> &'static str {
    env!("CARGO_BIN_EXE_harflow")
}

#[test]
fn doctor_json_exits_zero_with_expected_fields() {
    let out = Command::new(bin_path())
        .args(["doctor", "--json"])
        .output()
        .expect("run harflow doctor");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid doctor json");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["schemaVersion"], "1.0.0");
    assert_eq!(json["offlineDefault"], true);
}

#[test]
fn full_import_analyze_endpoints_graph_export_report_pipeline() {
    let project_dir = std::env::temp_dir().join(format!("harflow_cli_e2e_{}", Uuid::new_v4()));
    let fixture = repo_root().join("fixtures/har-valid/minimal-rest.har.json");

    let import = Command::new(bin_path())
        .args([
            "import",
            fixture.to_str().expect("fixture path"),
            "--project",
            project_dir.to_str().expect("project path"),
        ])
        .output()
        .expect("run import");
    assert!(
        import.status.success(),
        "import stderr: {}",
        String::from_utf8_lossy(&import.stderr)
    );
    assert!(project_dir.join("project.json").exists());
    assert!(project_dir.join("api-model.json").exists());
    assert!(project_dir.join("harflow.sqlite").exists());

    for args in [
        vec!["analyze", project_dir.to_str().unwrap()],
        vec!["endpoints", project_dir.to_str().unwrap(), "--json"],
        vec!["graph", project_dir.to_str().unwrap(), "--format", "json"],
        vec![
            "export",
            project_dir.to_str().unwrap(),
            "--format",
            "openapi",
        ],
        vec![
            "report",
            project_dir.to_str().unwrap(),
            "--format",
            "markdown",
        ],
        vec!["validate", project_dir.to_str().unwrap(), "--mode", "mock"],
    ] {
        let out = Command::new(bin_path())
            .args(&args)
            .output()
            .expect("run subcommand");
        assert!(
            out.status.success(),
            "command {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let _ = fs::remove_dir_all(&project_dir);
}

#[test]
fn malformed_har_exits_3_and_unknown_flag_exits_2() {
    let project_dir = std::env::temp_dir().join(format!("harflow_cli_bad_{}", Uuid::new_v4()));
    let bad_fixture = repo_root().join("fixtures/har-invalid/malformed-missing-log.har.json");

    let bad_import = Command::new(bin_path())
        .args([
            "import",
            bad_fixture.to_str().expect("bad fixture"),
            "--project",
            project_dir.to_str().expect("project path"),
        ])
        .output()
        .expect("run bad import");
    assert_eq!(bad_import.status.code(), Some(3));

    let bad_flag = Command::new(bin_path())
        .arg("--unknown-flag")
        .output()
        .expect("run unknown flag");
    assert_eq!(bad_flag.status.code(), Some(2));

    let _ = fs::remove_dir_all(&project_dir);
}

#[test]
fn multi_har_and_gzip_import_strict_and_config_validation() {
    let base = std::env::temp_dir().join(format!("harflow_cli_multi_{}", Uuid::new_v4()));
    let project_dir = base.join("proj-multi");
    let session_a = repo_root().join("fixtures/multi-session/session-a.har.json");
    let session_b = repo_root().join("fixtures/multi-session/session-b.har.json");

    let multi = Command::new(bin_path())
        .args([
            "import",
            session_a.to_str().unwrap(),
            session_b.to_str().unwrap(),
            "--project",
            project_dir.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run multi import");
    assert!(
        multi.status.success(),
        "multi stderr: {}",
        String::from_utf8_lossy(&multi.stderr)
    );
    let multi_json: serde_json::Value =
        serde_json::from_slice(&multi.stdout).expect("parse multi json");
    assert_eq!(multi_json["importedCount"], 2);

    let analyze = Command::new(bin_path())
        .args(["analyze", project_dir.to_str().unwrap(), "--json"])
        .output()
        .expect("run analyze");
    assert!(analyze.status.success());
    let stats: serde_json::Value = serde_json::from_slice(&analyze.stdout).expect("parse stats");
    assert_eq!(stats["totalRequests"], 2);

    // Gzipped HAR import test
    fs::create_dir_all(&base).expect("mkdir base");
    let gz_path = base.join("sample.har.gz");
    let raw_bytes = fs::read(&session_a).expect("read session_a");
    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    enc.write_all(&raw_bytes).expect("write gz");
    fs::write(&gz_path, enc.finish().expect("finish gz")).expect("write gz file");

    let gz_proj = base.join("proj-gz");
    let gz_out = Command::new(bin_path())
        .args([
            "import",
            gz_path.to_str().unwrap(),
            "--project",
            gz_proj.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("run gz import");
    assert!(
        gz_out.status.success(),
        "gz stderr: {}",
        String::from_utf8_lossy(&gz_out.stderr)
    );

    // Strict mode failure on partial entry
    let partial_har = base.join("partial.har.json");
    fs::write(
        &partial_har,
        r#"{"log":{"version":"1.2","entries":[{"request":{"method":"","url":""}},{"request":{"method":"GET","url":"https://api.example.test/ok"}}]}}"#,
    )
    .expect("write partial har");
    let strict_proj = base.join("proj-strict");
    let strict_out = Command::new(bin_path())
        .args([
            "import",
            partial_har.to_str().unwrap(),
            "--project",
            strict_proj.to_str().unwrap(),
            "--strict",
        ])
        .output()
        .expect("run strict import");
    assert_eq!(strict_out.status.code(), Some(3));

    // Missing --config exits with code 2 (InvalidArgs)
    let bad_cfg_out = Command::new(bin_path())
        .args([
            "import",
            session_a.to_str().unwrap(),
            "--project",
            base.join("proj-badcfg").to_str().unwrap(),
            "--config",
            base.join("missing-config.json").to_str().unwrap(),
        ])
        .output()
        .expect("run bad config");
    assert_eq!(bad_cfg_out.status.code(), Some(2));

    let _ = fs::remove_dir_all(&base);
}
