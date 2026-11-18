use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use harflow_project::ProjectWorkspace;
use serde_json::json;
use uuid::Uuid;

fn build_synthetic_har(count: usize) -> Vec<u8> {
    let mut out = String::with_capacity(count * 380);
    out.push_str(r#"{"log":{"version":"1.2","entries":["#);
    for i in 0..count {
        if i > 0 {
            out.push(',');
        }
        let kind = i % 2;
        let path = if kind == 0 {
            format!(
                "https://api.example.test/v1/users/{}?include=profile",
                100 + (i % 50)
            )
        } else {
            format!("https://api.example.test/v1/orders/{}", 1000 + i)
        };
        let entry = format!(
            r#"{{"startedDateTime":"2026-11-18T00:00:00Z","time":12.5,"request":{{"method":"GET","url":"{path}","headers":[{{"name":"Accept","value":"application/json"}},{{"name":"Authorization","value":"Bearer hf_synthetic_bench_token_{}"}}]}},"response":{{"status":200,"content":{{"mimeType":"application/json","text":"{{\"id\":{i},\"ok\":true}}"}}}}}}"#,
            i % 10
        );
        out.push_str(&entry);
    }
    out.push_str("]}}");
    out.into_bytes()
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn bench_count(count: usize, runs: usize) -> serde_json::Value {
    let payload = build_synthetic_har(count);
    let mut timings_ms = Vec::with_capacity(runs);

    for _ in 0..runs {
        let dir = std::env::temp_dir().join(format!("harflow_bench_{count}_{}", Uuid::new_v4()));
        let start = Instant::now();
        let mut ws =
            ProjectWorkspace::create(&dir, format!("Bench {count}")).expect("create bench");
        ws.import_har_reader("bench.har.json", &payload[..], None)
            .expect("import bench");
        ws.analyze().expect("analyze bench");
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        timings_ms.push((elapsed * 100.0).round() / 100.0);
        let _ = fs::remove_dir_all(&dir);
    }

    timings_ms.sort_by(f64::total_cmp);
    let p50 = percentile(&timings_ms, 0.50);
    let p95 = percentile(&timings_ms, 0.95);

    json!({
        "requests": count,
        "payloadBytes": payload.len(),
        "runs": runs,
        "timingsMs": timings_ms,
        "p50Ms": p50,
        "p95Ms": p95,
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let write_file = args.iter().any(|a| a == "--write");
    let custom_out = args
        .windows(2)
        .find(|w| w[0] == "--out")
        .map(|w| PathBuf::from(&w[1]));

    let r1k = bench_count(1_000, 5);
    let r10k = bench_count(10_000, 5);

    let report = json!({
        "phase": "Import, Redaction, Normalization, SQLite Index, Cleaner, Endpoint Discovery & Schema Inference",
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "results": [r1k, r10k],
    });

    let rendered = serde_json::to_string_pretty(&report).expect("serialize benchmark report");
    println!("{rendered}");

    if let Some(rel) = custom_out {
        let out_path = if rel.is_absolute() {
            rel
        } else {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .expect("crates")
                .parent()
                .expect("repo root")
                .join(rel)
        };
        fs::write(&out_path, format!("{rendered}\n")).expect("write benchmark result");
    } else if write_file {
        let out_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates")
            .parent()
            .expect("repo root")
            .join("benchmarks/results/phase1-baseline.json");
        fs::write(&out_path, format!("{rendered}\n")).expect("write benchmark result");
    }
}
