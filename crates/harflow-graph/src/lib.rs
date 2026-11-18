use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use harflow_core::SCHEMA_VERSION;
use harflow_model::{
    ApiModel, ConfirmationStatus, Dependency, DependencyType, Diagnostic, DiagnosticSeverity,
};
use harflow_normalizer::{NormalizedEntry, NormalizedSession};
use serde::{Deserialize, Serialize};

/// Supported graph perspectives specified in SRS M10.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphKind {
    RequestDependency,
    Endpoint,
    AuthFlow,
    SessionTimeline,
    DataFlow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub node_type: String,
    pub method: Option<String>,
    pub host: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub id: String,
    pub source_node_id: String,
    pub target_node_id: String,
    pub status: ConfirmationStatus,
    pub confidence: f64,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependency_type: Option<DependencyType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_pagination_loop: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphViewModel {
    pub schema_version: String,
    pub project_id: String,
    pub kind: GraphKind,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub has_cycles: bool,
}

/// Distinguishes logical endpoint pagination loops from true chronological/request-level contradictions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CycleAnalysis {
    pub has_endpoint_cycles: bool,
    pub pagination_loop_edge_ids: Vec<String>,
    pub contradiction_edge_ids: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Builds an endpoint-level graph view from the canonical `ApiModel`.
#[must_use]
pub fn build_endpoint_graph(model: &ApiModel) -> GraphViewModel {
    build_graph(model, &[], GraphKind::Endpoint)
}

/// Builds any of the 5 SRS M10 graph perspectives from the canonical `ApiModel` and sessions.
#[must_use]
pub fn build_graph(
    model: &ApiModel,
    sessions: &[NormalizedSession],
    kind: GraphKind,
) -> GraphViewModel {
    match kind {
        GraphKind::Endpoint | GraphKind::DataFlow => build_endpoint_or_dataflow_graph(model, kind),
        GraphKind::RequestDependency => build_request_dependency_graph(model, sessions),
        GraphKind::AuthFlow => build_auth_flow_graph(model, sessions),
        GraphKind::SessionTimeline => build_session_timeline_graph(model, sessions),
    }
}

fn build_endpoint_or_dataflow_graph(model: &ApiModel, kind: GraphKind) -> GraphViewModel {
    let mut request_to_endpoint = BTreeMap::new();
    let mut nodes = Vec::with_capacity(model.endpoints.len());

    for ep in &model.endpoints {
        nodes.push(GraphNode {
            id: ep.id.clone(),
            label: format!("{} {}", ep.method, ep.path_template),
            node_type: "endpoint".to_string(),
            method: Some(ep.method.clone()),
            host: Some(ep.host.clone()),
        });
        for req_id in &ep.observed_request_ids {
            request_to_endpoint.insert(req_id.as_str(), ep.id.clone());
        }
    }

    let mut edges = Vec::new();
    for dep in &model.dependencies {
        let source_node = request_to_endpoint
            .get(dep.source.request_id.as_str())
            .cloned()
            .unwrap_or_else(|| dep.source.request_id.clone());
        let target_node = request_to_endpoint
            .get(dep.target.request_id.as_str())
            .cloned()
            .unwrap_or_else(|| dep.target.request_id.clone());

        let is_pag_loop =
            dep.dependency_type == DependencyType::PaginationChain && source_node == target_node;
        let label = match kind {
            GraphKind::DataFlow => format!(
                "{:?}:{} -> {:?}:{}",
                dep.source.location,
                dep.source.json_path_or_key,
                dep.target.location,
                dep.target.json_path_or_key
            ),
            _ => format!(
                "{} -> {}",
                dep.source.json_path_or_key, dep.target.json_path_or_key
            ),
        };

        edges.push(GraphEdge {
            id: dep.id.clone(),
            source_node_id: source_node,
            target_node_id: target_node,
            status: dep.status,
            confidence: dep.confidence,
            label,
            dependency_type: Some(dep.dependency_type.clone()),
            is_pagination_loop: Some(is_pag_loop),
        });
    }

    let has_cycles = has_directed_cycle(&nodes, &edges);

    GraphViewModel {
        schema_version: SCHEMA_VERSION.to_string(),
        project_id: model.project_id.clone(),
        kind,
        nodes,
        edges,
        has_cycles,
    }
}

fn build_request_dependency_graph(
    model: &ApiModel,
    sessions: &[NormalizedSession],
) -> GraphViewModel {
    let mut node_map: BTreeMap<String, GraphNode> = BTreeMap::new();
    for s in sessions {
        for e in &s.entries {
            node_map.insert(e.id.clone(), request_node(e));
        }
    }
    for dep in &model.dependencies {
        for req_id in [&dep.source.request_id, &dep.target.request_id] {
            node_map.entry(req_id.clone()).or_insert_with(|| GraphNode {
                id: req_id.clone(),
                label: req_id.clone(),
                node_type: "request".to_string(),
                method: None,
                host: None,
            });
        }
    }

    let nodes: Vec<GraphNode> = node_map.into_values().collect();
    let edges: Vec<GraphEdge> = model
        .dependencies
        .iter()
        .map(|dep| GraphEdge {
            id: dep.id.clone(),
            source_node_id: dep.source.request_id.clone(),
            target_node_id: dep.target.request_id.clone(),
            status: dep.status,
            confidence: dep.confidence,
            label: format!(
                "{} -> {}",
                dep.source.json_path_or_key, dep.target.json_path_or_key
            ),
            dependency_type: Some(dep.dependency_type.clone()),
            is_pagination_loop: Some(dep.dependency_type == DependencyType::PaginationChain),
        })
        .collect();

    let has_cycles = has_directed_cycle(&nodes, &edges);
    GraphViewModel {
        schema_version: SCHEMA_VERSION.to_string(),
        project_id: model.project_id.clone(),
        kind: GraphKind::RequestDependency,
        nodes,
        edges,
        has_cycles,
    }
}

fn build_auth_flow_graph(model: &ApiModel, sessions: &[NormalizedSession]) -> GraphViewModel {
    let mut node_map: BTreeMap<String, GraphNode> = BTreeMap::new();
    let mut req_to_ep = BTreeMap::new();
    for ep in &model.endpoints {
        node_map.insert(
            ep.id.clone(),
            GraphNode {
                id: ep.id.clone(),
                label: format!("{} {}", ep.method, ep.path_template),
                node_type: "endpoint".to_string(),
                method: Some(ep.method.clone()),
                host: Some(ep.host.clone()),
            },
        );
        for req_id in &ep.observed_request_ids {
            req_to_ep.insert(req_id.as_str(), ep.id.clone());
        }
    }

    let mut edges = Vec::new();
    for flow in &model.auth_flows {
        node_map.insert(
            flow.id.clone(),
            GraphNode {
                id: flow.id.clone(),
                label: format!("{:?}: {}", flow.scheme, flow.id),
                node_type: "auth_flow".to_string(),
                method: None,
                host: None,
            },
        );

        let mut connected_endpoints = BTreeSet::new();
        for req_id in &flow.request_ids {
            if let Some(ep_id) = req_to_ep.get(req_id.as_str()) {
                connected_endpoints.insert(ep_id.clone());
            } else if sessions
                .iter()
                .any(|s| s.entries.iter().any(|e| &e.id == req_id))
            {
                connected_endpoints.insert(req_id.clone());
            }
        }
        for target_id in connected_endpoints {
            edges.push(GraphEdge {
                id: format!("edge-{}-{}", flow.id, target_id),
                source_node_id: flow.id.clone(),
                target_node_id: target_id,
                status: ConfirmationStatus::Confirmed,
                confidence: flow.confidence,
                label: format!("{:?}", flow.scheme),
                dependency_type: Some(DependencyType::SessionChain),
                is_pagination_loop: Some(false),
            });
        }
    }

    for dep in &model.dependencies {
        if matches!(
            dep.dependency_type,
            DependencyType::SessionChain | DependencyType::SetCookieToCookie
        ) {
            let src = req_to_ep
                .get(dep.source.request_id.as_str())
                .cloned()
                .unwrap_or_else(|| dep.source.request_id.clone());
            let dst = req_to_ep
                .get(dep.target.request_id.as_str())
                .cloned()
                .unwrap_or_else(|| dep.target.request_id.clone());
            edges.push(GraphEdge {
                id: dep.id.clone(),
                source_node_id: src,
                target_node_id: dst,
                status: dep.status,
                confidence: dep.confidence,
                label: format!(
                    "{} -> {}",
                    dep.source.json_path_or_key, dep.target.json_path_or_key
                ),
                dependency_type: Some(dep.dependency_type.clone()),
                is_pagination_loop: Some(false),
            });
        }
    }

    let nodes: Vec<GraphNode> = node_map.into_values().collect();
    let has_cycles = has_directed_cycle(&nodes, &edges);
    GraphViewModel {
        schema_version: SCHEMA_VERSION.to_string(),
        project_id: model.project_id.clone(),
        kind: GraphKind::AuthFlow,
        nodes,
        edges,
        has_cycles,
    }
}

fn build_session_timeline_graph(
    model: &ApiModel,
    sessions: &[NormalizedSession],
) -> GraphViewModel {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for session in sessions {
        for (idx, entry) in session.entries.iter().enumerate() {
            nodes.push(request_node(entry));
            if idx > 0 {
                let prev = &session.entries[idx - 1];
                edges.push(GraphEdge {
                    id: format!("timeline-{}-{}", prev.id, entry.id),
                    source_node_id: prev.id.clone(),
                    target_node_id: entry.id.clone(),
                    status: ConfirmationStatus::Confirmed,
                    confidence: 1.0,
                    label: "timeline".to_string(),
                    dependency_type: None,
                    is_pagination_loop: Some(false),
                });
            }
        }
    }

    for dep in &model.dependencies {
        edges.push(GraphEdge {
            id: dep.id.clone(),
            source_node_id: dep.source.request_id.clone(),
            target_node_id: dep.target.request_id.clone(),
            status: dep.status,
            confidence: dep.confidence,
            label: format!(
                "{} -> {}",
                dep.source.json_path_or_key, dep.target.json_path_or_key
            ),
            dependency_type: Some(dep.dependency_type.clone()),
            is_pagination_loop: Some(dep.dependency_type == DependencyType::PaginationChain),
        });
    }

    let has_cycles = has_directed_cycle(&nodes, &edges);
    GraphViewModel {
        schema_version: SCHEMA_VERSION.to_string(),
        project_id: model.project_id.clone(),
        kind: GraphKind::SessionTimeline,
        nodes,
        edges,
        has_cycles,
    }
}

fn request_node(entry: &NormalizedEntry) -> GraphNode {
    GraphNode {
        id: entry.id.clone(),
        label: format!("{} {}", entry.method, entry.path),
        node_type: "request".to_string(),
        method: Some(entry.method.clone()),
        host: Some(entry.host.clone()),
    }
}

/// Separates expected endpoint-level pagination loops from invalid request-level cycle contradictions.
#[must_use]
pub fn analyze_graph_cycles(model: &ApiModel) -> CycleAnalysis {
    let active_deps: Vec<&Dependency> = model
        .dependencies
        .iter()
        .filter(|d| d.status != ConfirmationStatus::Rejected)
        .collect();

    let ep_graph = build_endpoint_graph(model);
    let active_ep_edges: Vec<GraphEdge> = ep_graph
        .edges
        .into_iter()
        .filter(|e| e.status != ConfirmationStatus::Rejected)
        .collect();
    let has_endpoint_cycles = has_directed_cycle(&ep_graph.nodes, &active_ep_edges);

    let pagination_loop_edge_ids: Vec<String> = active_ep_edges
        .iter()
        .filter(|e| {
            e.is_pagination_loop == Some(true)
                || (e.source_node_id == e.target_node_id
                    && e.dependency_type == Some(DependencyType::PaginationChain))
        })
        .map(|e| e.id.clone())
        .collect();

    let contradiction_edge_ids = find_request_cycle_contradictions(&active_deps);
    let diagnostics = if contradiction_edge_ids.is_empty() {
        Vec::new()
    } else {
        vec![Diagnostic {
            code: "DEPENDENCY_CYCLE_CONTRADICTION".to_string(),
            severity: DiagnosticSeverity::Warning,
            message: format!(
                "Detected {} contradictory request-level dependency edge(s) forming a directed cycle",
                contradiction_edge_ids.len()
            ),
            related_entity_ids: contradiction_edge_ids.clone(),
        }]
    };

    CycleAnalysis {
        has_endpoint_cycles,
        pagination_loop_edge_ids,
        contradiction_edge_ids,
        diagnostics,
    }
}

fn find_request_cycle_contradictions(deps: &[&Dependency]) -> Vec<String> {
    if deps.is_empty() {
        return Vec::new();
    }

    let mut nodes: BTreeSet<&str> = BTreeSet::new();
    let mut in_deg: HashMap<&str, usize> = HashMap::new();
    let mut out_deg: HashMap<&str, usize> = HashMap::new();
    let mut out_adj: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut in_adj: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut self_loop_ids = BTreeSet::new();

    for dep in deps {
        let src = dep.source.request_id.as_str();
        let dst = dep.target.request_id.as_str();
        nodes.insert(src);
        nodes.insert(dst);
        if src == dst {
            self_loop_ids.insert(dep.id.clone());
            continue;
        }
        out_adj.entry(src).or_default().push(dst);
        in_adj.entry(dst).or_default().push(src);
        *out_deg.entry(src).or_insert(0) += 1;
        *in_deg.entry(dst).or_insert(0) += 1;
    }

    // Peel off nodes with in_degree == 0 or out_degree == 0 iteratively; only true cycle core remains.
    let mut alive: BTreeSet<&str> = nodes;
    let mut queue = VecDeque::new();
    for &n in &alive {
        if in_deg.get(n).copied().unwrap_or(0) == 0 || out_deg.get(n).copied().unwrap_or(0) == 0 {
            queue.push_back(n);
        }
    }

    while let Some(n) = queue.pop_front() {
        if !alive.remove(n) {
            continue;
        }
        if let Some(nexts) = out_adj.get(n) {
            for &nxt in nexts {
                if alive.contains(nxt) {
                    if let Some(d) = in_deg.get_mut(nxt) {
                        *d = d.saturating_sub(1);
                        if *d == 0 {
                            queue.push_back(nxt);
                        }
                    }
                }
            }
        }
        if let Some(prevs) = in_adj.get(n) {
            for &prv in prevs {
                if alive.contains(prv) {
                    if let Some(d) = out_deg.get_mut(prv) {
                        *d = d.saturating_sub(1);
                        if *d == 0 {
                            queue.push_back(prv);
                        }
                    }
                }
            }
        }
    }

    let mut out = self_loop_ids;
    for dep in deps {
        if alive.contains(dep.source.request_id.as_str())
            && alive.contains(dep.target.request_id.as_str())
        {
            out.insert(dep.id.clone());
        }
    }
    out.into_iter().collect()
}

/// Checks whether a directed graph contains at least one cycle using Kahn's topological sort.
#[must_use]
pub fn has_directed_cycle(nodes: &[GraphNode], edges: &[GraphEdge]) -> bool {
    let mut all_nodes: BTreeSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let mut in_degree: BTreeMap<&str, usize> = BTreeMap::new();
    let mut adj: BTreeMap<&str, Vec<&str>> = BTreeMap::new();

    for edge in edges {
        all_nodes.insert(edge.source_node_id.as_str());
        all_nodes.insert(edge.target_node_id.as_str());
        adj.entry(edge.source_node_id.as_str())
            .or_default()
            .push(edge.target_node_id.as_str());
        *in_degree.entry(edge.target_node_id.as_str()).or_insert(0) += 1;
    }

    let mut queue = VecDeque::new();
    for &node in &all_nodes {
        if in_degree.get(node).copied().unwrap_or(0) == 0 {
            queue.push_back(node);
        }
    }

    let mut visited = 0usize;
    while let Some(node) = queue.pop_front() {
        visited += 1;
        if let Some(neighbors) = adj.get(node) {
            for &next in neighbors {
                if let Some(deg) = in_degree.get_mut(next) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(next);
                    }
                }
            }
        }
    }

    visited < all_nodes.len()
}

#[cfg(test)]
mod tests {
    use harflow_model::{DependencyType, Endpoint, ValueLocation, ValueReference};

    use super::*;

    fn node(id: &str) -> GraphNode {
        GraphNode {
            id: id.into(),
            label: id.into(),
            node_type: "endpoint".into(),
            method: Some("GET".into()),
            host: Some("api.example.test".into()),
        }
    }

    fn edge(id: &str, src: &str, dst: &str) -> GraphEdge {
        GraphEdge {
            id: id.into(),
            source_node_id: src.into(),
            target_node_id: dst.into(),
            status: ConfirmationStatus::Confirmed,
            confidence: 0.95,
            label: "cursor".into(),
            dependency_type: None,
            is_pagination_loop: None,
        }
    }

    #[test]
    fn distinguishes_acyclic_chain_from_pagination_cycle() {
        let nodes = vec![node("a"), node("b")];
        let acyclic = vec![edge("e1", "a", "b")];
        assert!(!has_directed_cycle(&nodes, &acyclic));

        let cyclic = vec![edge("e1", "a", "b"), edge("e2", "b", "a")];
        assert!(has_directed_cycle(&nodes, &cyclic));
    }

    #[test]
    fn separates_valid_pagination_loop_from_request_contradiction() {
        let mut model = ApiModel::empty("p1", "Test");
        model.endpoints.push(Endpoint {
            id: "ep-products".into(),
            operation_id: None,
            method: "GET".into(),
            host: "api.example.test".into(),
            path_template: "/v1/products".into(),
            parameters: Vec::new(),
            request_schema_id: None,
            response_schema_id: None,
            status_codes: vec![200],
            status_distribution: BTreeMap::new(),
            pagination: None,
            confidence: 0.9,
            category: "api".into(),
            observed_request_ids: vec!["req-1".into(), "req-2".into()],
        });
        model.dependencies.push(Dependency {
            id: "dep-pag".into(),
            source: ValueReference {
                session_id: "s1".into(),
                request_id: "req-1".into(),
                location: ValueLocation::ResponseBody,
                json_path_or_key: "$.nextCursor".into(),
            },
            target: ValueReference {
                session_id: "s1".into(),
                request_id: "req-2".into(),
                location: ValueLocation::Query,
                json_path_or_key: "cursor".into(),
            },
            dependency_type: DependencyType::PaginationChain,
            transformation: Some(harflow_model::Transformation::Identity),
            confidence: 0.95,
            status: ConfirmationStatus::Confirmed,
            evidence: Vec::new(),
            observed_session_ids: vec!["s1".into()],
        });

        let analysis = analyze_graph_cycles(&model);
        assert!(analysis.has_endpoint_cycles);
        assert_eq!(analysis.pagination_loop_edge_ids, vec!["dep-pag"]);
        assert!(analysis.contradiction_edge_ids.is_empty());
        assert!(analysis.diagnostics.is_empty());

        // Inject reverse request edge req-2 -> req-1 -> request-level cycle contradiction!
        model.dependencies.push(Dependency {
            id: "dep-bad-rev".into(),
            source: ValueReference {
                session_id: "s1".into(),
                request_id: "req-2".into(),
                location: ValueLocation::ResponseBody,
                json_path_or_key: "$.id".into(),
            },
            target: ValueReference {
                session_id: "s1".into(),
                request_id: "req-1".into(),
                location: ValueLocation::Query,
                json_path_or_key: "ref".into(),
            },
            dependency_type: DependencyType::ResponseToQuery,
            transformation: Some(harflow_model::Transformation::Identity),
            confidence: 0.70,
            status: ConfirmationStatus::Likely,
            evidence: Vec::new(),
            observed_session_ids: vec!["s1".into()],
        });

        let bad = analyze_graph_cycles(&model);
        assert_eq!(bad.contradiction_edge_ids.len(), 2);
        assert_eq!(bad.diagnostics[0].code, "DEPENDENCY_CYCLE_CONTRADICTION");
    }
}
