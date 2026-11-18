use std::collections::{BTreeMap, HashMap};

use harflow_model::{
    ConfirmationStatus, Dependency, DependencyType, Endpoint, Transformation, ValueLocation,
};
use harflow_normalizer::NormalizedSession;
use serde::{Deserialize, Serialize};

use crate::detect_dependencies;

/// Expected ground-truth direct dependency for deterministic benchmark evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpectedDependency {
    pub source_endpoint: String,
    pub source_location: ValueLocation,
    pub source_key: String,
    pub target_endpoint: String,
    pub target_location: ValueLocation,
    pub target_key: String,
    pub dependency_type: DependencyType,
    #[serde(default)]
    pub transformation: Option<Transformation>,
}

/// Evaluation metrics for one labeled multi-request/multi-session scenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioEvaluationResult {
    pub scenario_id: String,
    pub ground_truth_total: usize,
    pub detected_total: usize,
    pub true_positives_confirmed: usize,
    pub false_positives_confirmed: usize,
    pub true_positives_detected: usize,
    pub un_evidenced_confirmed: usize,
    pub confirmed_precision: f64,
    pub direct_recall: f64,
    pub missed_ground_truth: Vec<String>,
    pub unexpected_confirmed: Vec<String>,
}

/// Aggregate precision, recall, and F1 report across all scenarios in a labeled dataset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetEvaluationSummary {
    pub dataset_name: String,
    pub scenarios: Vec<ScenarioEvaluationResult>,
    pub ground_truth_total: usize,
    pub detected_total: usize,
    pub true_positives_confirmed: usize,
    pub false_positives_confirmed: usize,
    pub true_positives_detected: usize,
    pub un_evidenced_confirmed: usize,
    pub confirmed_precision: f64,
    pub direct_recall: f64,
    pub f1: f64,
}

/// Evaluates `detect_dependencies` on a single labeled scenario against `expected` edges.
#[must_use]
pub fn evaluate_scenario(
    scenario_id: &str,
    sessions: &[NormalizedSession],
    endpoints: &[Endpoint],
    expected: &[ExpectedDependency],
) -> ScenarioEvaluationResult {
    let detected = detect_dependencies(sessions, endpoints);
    let req_to_ep = build_req_to_ep_signature(sessions, endpoints);

    let mut matched_expected = vec![false; expected.len()];
    let mut matched_detected = vec![false; detected.len()];

    // Prioritize higher-confidence predictions during greedy 1-to-1 ground-truth matching.
    for (d_idx, dep) in detected.iter().enumerate() {
        for (e_idx, exp) in expected.iter().enumerate() {
            if matched_expected[e_idx] {
                continue;
            }
            if dependency_matches_expected(dep, exp, &req_to_ep) {
                matched_expected[e_idx] = true;
                matched_detected[d_idx] = true;
                break;
            }
        }
    }

    let mut tp_confirmed = 0usize;
    let mut fp_confirmed = 0usize;
    let mut tp_detected = 0usize;
    let mut un_evidenced_confirmed = 0usize;
    let mut unexpected_confirmed = Vec::new();

    for (d_idx, dep) in detected.iter().enumerate() {
        let is_match = matched_detected[d_idx];
        if dep.status == ConfirmationStatus::Confirmed {
            if dep.evidence.is_empty() {
                un_evidenced_confirmed += 1;
            }
            if is_match {
                tp_confirmed += 1;
            } else {
                fp_confirmed += 1;
                let src_ep = req_to_ep
                    .get(dep.source.request_id.as_str())
                    .map_or("?", String::as_str);
                let dst_ep = req_to_ep
                    .get(dep.target.request_id.as_str())
                    .map_or("?", String::as_str);
                unexpected_confirmed.push(format!(
                    "{} ({src_ep} {:?}:{} -> {dst_ep} {:?}:{} [{:?}/{:?}] conf={})",
                    dep.id,
                    dep.source.location,
                    dep.source.json_path_or_key,
                    dep.target.location,
                    dep.target.json_path_or_key,
                    dep.dependency_type,
                    dep.transformation,
                    dep.confidence
                ));
            }
        }
        if is_match
            && matches!(
                dep.status,
                ConfirmationStatus::Confirmed
                    | ConfirmationStatus::Likely
                    | ConfirmationStatus::Unknown
            )
        {
            tp_detected += 1;
        }
    }

    let missed_ground_truth: Vec<String> = expected
        .iter()
        .enumerate()
        .filter(|(idx, _)| !matched_expected[*idx])
        .map(|(_, exp)| {
            format!(
                "{} {:?}:{} -> {} {:?}:{} ({:?})",
                exp.source_endpoint,
                exp.source_location,
                exp.source_key,
                exp.target_endpoint,
                exp.target_location,
                exp.target_key,
                exp.dependency_type
            )
        })
        .collect();

    let total_confirmed = tp_confirmed + fp_confirmed;
    let confirmed_precision = if total_confirmed == 0 {
        1.0
    } else {
        tp_confirmed as f64 / total_confirmed as f64
    };
    let direct_recall = if expected.is_empty() {
        1.0
    } else {
        tp_detected as f64 / expected.len() as f64
    };

    ScenarioEvaluationResult {
        scenario_id: scenario_id.to_string(),
        ground_truth_total: expected.len(),
        detected_total: detected.len(),
        true_positives_confirmed: tp_confirmed,
        false_positives_confirmed: fp_confirmed,
        true_positives_detected: tp_detected,
        un_evidenced_confirmed,
        confirmed_precision: round4(confirmed_precision),
        direct_recall: round4(direct_recall),
        missed_ground_truth,
        unexpected_confirmed,
    }
}

/// Evaluates multiple labeled scenarios and computes aggregate Precision, Recall, and F1.
#[must_use]
pub fn summarize_dataset_evaluation(
    dataset_name: &str,
    scenarios: Vec<ScenarioEvaluationResult>,
) -> DatasetEvaluationSummary {
    let ground_truth_total: usize = scenarios.iter().map(|s| s.ground_truth_total).sum();
    let detected_total: usize = scenarios.iter().map(|s| s.detected_total).sum();
    let tp_confirmed: usize = scenarios.iter().map(|s| s.true_positives_confirmed).sum();
    let fp_confirmed: usize = scenarios.iter().map(|s| s.false_positives_confirmed).sum();
    let tp_detected: usize = scenarios.iter().map(|s| s.true_positives_detected).sum();
    let un_evidenced_confirmed: usize = scenarios.iter().map(|s| s.un_evidenced_confirmed).sum();

    let confirmed_precision = if tp_confirmed + fp_confirmed == 0 {
        1.0
    } else {
        tp_confirmed as f64 / (tp_confirmed + fp_confirmed) as f64
    };
    let direct_recall = if ground_truth_total == 0 {
        1.0
    } else {
        tp_detected as f64 / ground_truth_total as f64
    };
    let f1 = if confirmed_precision + direct_recall == 0.0 {
        0.0
    } else {
        2.0 * confirmed_precision * direct_recall / (confirmed_precision + direct_recall)
    };

    DatasetEvaluationSummary {
        dataset_name: dataset_name.to_string(),
        scenarios,
        ground_truth_total,
        detected_total,
        true_positives_confirmed: tp_confirmed,
        false_positives_confirmed: fp_confirmed,
        true_positives_detected: tp_detected,
        un_evidenced_confirmed,
        confirmed_precision: round4(confirmed_precision),
        direct_recall: round4(direct_recall),
        f1: round4(f1),
    }
}

fn build_req_to_ep_signature(
    sessions: &[NormalizedSession],
    endpoints: &[Endpoint],
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for s in sessions {
        for e in &s.entries {
            map.insert(e.id.clone(), format!("{} {}", e.method, e.path));
        }
    }
    for ep in endpoints {
        let sig = format!("{} {}", ep.method, ep.path_template);
        for req_id in &ep.observed_request_ids {
            map.insert(req_id.clone(), sig.clone());
        }
    }
    let _ordered_guard: BTreeMap<(), ()> = BTreeMap::new();
    map
}

fn dependency_matches_expected(
    dep: &Dependency,
    exp: &ExpectedDependency,
    req_to_ep: &HashMap<String, String>,
) -> bool {
    if dep.dependency_type != exp.dependency_type
        || dep.source.location != exp.source_location
        || dep.target.location != exp.target_location
    {
        return false;
    }

    if let Some(expected_trans) = &exp.transformation {
        if dep.transformation.as_ref() != Some(expected_trans) {
            return false;
        }
    }

    let Some(src_ep) = req_to_ep.get(&dep.source.request_id) else {
        return false;
    };
    let Some(dst_ep) = req_to_ep.get(&dep.target.request_id) else {
        return false;
    };
    if src_ep != &exp.source_endpoint || dst_ep != &exp.target_endpoint {
        return false;
    }

    normalize_locator(&dep.source.json_path_or_key) == normalize_locator(&exp.source_key)
        && normalize_locator(&dep.target.json_path_or_key) == normalize_locator(&exp.target_key)
}

fn normalize_locator(raw: &str) -> String {
    let trimmed = raw.trim().to_ascii_lowercase();
    // Collapse concrete array indices like $[0] / .items[0] into [*] so expectations are index-agnostic.
    let mut out = String::with_capacity(trimmed.len());
    let bytes = trimmed.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            out.push_str("[*]");
            i += 1;
            while i < bytes.len() && bytes[i] != b']' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
            }
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn round4(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}
