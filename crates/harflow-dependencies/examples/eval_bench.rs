use std::fs;
use std::path::PathBuf;

use harflow_dependencies::{
    evaluate_scenario, summarize_dataset_evaluation, DatasetEvaluationSummary, ExpectedDependency,
};
use harflow_endpoints::discover_endpoints;
use harflow_har::{parse_har_bytes, ImportLimits};
use harflow_normalizer::normalize_har_session;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LabeledDatasetFile {
    name: String,
    scenarios: Vec<LabeledScenarioInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LabeledScenarioInput {
    id: String,
    sessions: Vec<ScenarioSessionInput>,
    expected_dependencies: Vec<ExpectedDependency>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScenarioSessionInput {
    session_id: String,
    source_name: String,
    har: serde_json::Value,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates dir")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn run_dataset(relative_path: &str) -> DatasetEvaluationSummary {
    let path = repo_root().join(relative_path);
    let raw = fs::read_to_string(&path).expect("read dataset json");
    let dataset: LabeledDatasetFile = serde_json::from_str(&raw).expect("parse dataset");

    let mut scenario_results = Vec::with_capacity(dataset.scenarios.len());
    for sc in dataset.scenarios {
        let mut sessions = Vec::with_capacity(sc.sessions.len());
        for s in sc.sessions {
            let har_bytes = serde_json::to_vec(&s.har).expect("serialize har");
            let har = parse_har_bytes(&har_bytes, &ImportLimits::default()).expect("parse har");
            sessions.push(normalize_har_session(&s.session_id, &s.source_name, &har));
        }
        let endpoints = discover_endpoints(&sessions);
        scenario_results.push(evaluate_scenario(
            &sc.id,
            &sessions,
            &endpoints,
            &sc.expected_dependencies,
        ));
    }

    summarize_dataset_evaluation(&dataset.name, scenario_results)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let custom_out = args
        .windows(2)
        .find(|w| w[0] == "--out")
        .map(|w| PathBuf::from(&w[1]));

    let primary = run_dataset("benchmarks/datasets/dependency-eval-primary.json");
    let holdout = run_dataset("benchmarks/datasets/dependency-eval-holdout.json");

    let report = serde_json::json!({
        "phase": "Phase 3 Dependency Evaluation (Primary & Unseen Holdout)",
        "primary": primary,
        "holdout": holdout,
    });
    let rendered = serde_json::to_string_pretty(&report).expect("serialize eval report");
    println!("{rendered}");

    if let Some(rel) = custom_out {
        let out_path = if rel.is_absolute() {
            rel
        } else {
            repo_root().join(rel)
        };
        fs::write(&out_path, format!("{rendered}\n")).expect("write eval result");
    }
}
