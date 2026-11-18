use std::fs;
use std::path::PathBuf;
use std::process::Command;

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
