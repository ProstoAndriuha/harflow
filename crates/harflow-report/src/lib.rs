use std::collections::BTreeSet;

use chrono::Utc;
use harflow_core::SCHEMA_VERSION;
use harflow_model::{ApiModel, ConfirmationStatus, Diagnostic};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportReadiness {
    NotReady,
    Partial,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportTotals {
    pub requests: usize,
    pub endpoints: usize,
    pub domains: usize,
    pub dependencies: usize,
    pub auth_flows: usize,
    pub unknown_mappings: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisReport {
    pub schema_version: String,
    pub project_id: String,
    pub generated_at: String,
    pub totals: ReportTotals,
    pub export_readiness: ExportReadiness,
    pub diagnostics: Vec<Diagnostic>,
}

/// Builds a structured analysis report matching `schemas/analysis-report.schema.json`.
#[must_use]
pub fn build_report(model: &ApiModel) -> AnalysisReport {
    let domains: BTreeSet<&str> = model.endpoints.iter().map(|e| e.host.as_str()).collect();
    let unknown_mappings = model
        .dependencies
        .iter()
        .filter(|d| d.status == ConfirmationStatus::Unknown)
        .count();

    let readiness = if model.endpoints.is_empty() {
        ExportReadiness::NotReady
    } else if unknown_mappings > 0 {
        ExportReadiness::Partial
    } else {
        ExportReadiness::Ready
    };

    AnalysisReport {
        schema_version: SCHEMA_VERSION.to_string(),
        project_id: model.project_id.clone(),
        generated_at: Utc::now().to_rfc3339(),
        totals: ReportTotals {
            requests: model.metadata.total_requests,
            endpoints: model.endpoints.len(),
            domains: domains.len(),
            dependencies: model.dependencies.len(),
            auth_flows: model.auth_flows.len(),
            unknown_mappings,
        },
        export_readiness: readiness,
        diagnostics: model.diagnostics.clone(),
    }
}

/// Renders a concise Markdown report suitable for CLI output and documentation exports.
#[must_use]
pub fn render_markdown_report(report: &AnalysisReport) -> String {
    format!(
        "# HARFlow Analysis Report\n\n\
        - **Schema Version:** `{}`\n\
        - **Project ID:** `{}`\n\
        - **Generated At:** `{}`\n\
        - **Export Readiness:** `{:?}`\n\n\
        ## Summary Metrics\n\n\
        | Metric | Count |\n\
        | ------ | ----- |\n\
        | Total Requests | {} |\n\
        | Unique Endpoints | {} |\n\
        | API Domains | {} |\n\
        | Dependencies | {} |\n\
        | Auth Flows | {} |\n\
        | Unknown Mappings | {} |\n",
        report.schema_version,
        report.project_id,
        report.generated_at,
        report.export_readiness,
        report.totals.requests,
        report.totals.endpoints,
        report.totals.domains,
        report.totals.dependencies,
        report.totals.auth_flows,
        report.totals.unknown_mappings,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_and_renders_markdown_report() {
        let model = ApiModel::empty("proj-rpt", "Report Test");
        let report = build_report(&model);
        assert_eq!(report.export_readiness, ExportReadiness::NotReady);
        let md = render_markdown_report(&report);
        assert!(md.contains("HARFlow Analysis Report"));
        assert!(md.contains("proj-rpt"));
    }
}
