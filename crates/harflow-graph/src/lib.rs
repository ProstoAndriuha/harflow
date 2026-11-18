use std::collections::{BTreeMap, BTreeSet, VecDeque};

use harflow_core::SCHEMA_VERSION;
use harflow_model::{ApiModel, ConfirmationStatus};
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

/// Builds an endpoint-level graph view from the canonical `ApiModel`.
#[must_use]
pub fn build_endpoint_graph(model: &ApiModel) -> GraphViewModel {
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
            request_to_endpoint.insert(req_id.clone(), ep.id.clone());
        }
    }

    let mut edges = Vec::new();
    for dep in &model.dependencies {
        let source_node = request_to_endpoint
            .get(&dep.source.request_id)
            .cloned()
            .unwrap_or_else(|| dep.source.request_id.clone());
        let target_node = request_to_endpoint
            .get(&dep.target.request_id)
            .cloned()
            .unwrap_or_else(|| dep.target.request_id.clone());

        edges.push(GraphEdge {
            id: dep.id.clone(),
            source_node_id: source_node,
            target_node_id: target_node,
            status: dep.status,
            confidence: dep.confidence,
            label: format!(
                "{} -> {}",
                dep.source.json_path_or_key, dep.target.json_path_or_key
            ),
        });
    }

    let has_cycles = has_directed_cycle(&nodes, &edges);

    GraphViewModel {
        schema_version: SCHEMA_VERSION.to_string(),
        project_id: model.project_id.clone(),
        kind: GraphKind::Endpoint,
        nodes,
        edges,
        has_cycles,
    }
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
}
