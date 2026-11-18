use std::collections::BTreeMap;

use harflow_model::ApiModel;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Minimal OpenAPI 3.1.0 document skeleton generated from `ApiModel`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenApiDocument {
    pub openapi: String,
    pub info: OpenApiInfo,
    pub paths: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenApiInfo {
    pub title: String,
    pub version: String,
}

/// Builds an OpenAPI 3.1.0 specification from discovered endpoints.
#[must_use]
pub fn build_openapi_document(model: &ApiModel) -> OpenApiDocument {
    let mut paths: BTreeMap<String, Value> = BTreeMap::new();

    for ep in &model.endpoints {
        let entry = paths
            .entry(ep.path_template.clone())
            .or_insert_with(|| json!({}));
        if let Some(obj) = entry.as_object_mut() {
            let method_key = ep.method.to_ascii_lowercase();
            let mut responses = serde_json::Map::new();
            if ep.status_codes.is_empty() {
                responses.insert(
                    "200".to_string(),
                    json!({ "description": "Observed response" }),
                );
            } else {
                for code in &ep.status_codes {
                    responses.insert(
                        code.to_string(),
                        json!({ "description": "Observed status response" }),
                    );
                }
            }
            obj.insert(
                method_key,
                json!({
                    "operationId": ep.operation_id,
                    "responses": responses,
                }),
            );
        }
    }

    OpenApiDocument {
        openapi: "3.1.0".to_string(),
        info: OpenApiInfo {
            title: model.metadata.project_name.clone(),
            version: model.schema_version.clone(),
        },
        paths,
    }
}

#[cfg(test)]
mod tests {
    use harflow_model::Endpoint;

    use super::*;

    #[test]
    fn builds_openapi_31_document_with_paths() {
        let mut model = ApiModel::empty("p1", "Orders API");
        model.endpoints.push(Endpoint {
            id: "ep-1".into(),
            operation_id: Some("get_orders".into()),
            method: "GET".into(),
            host: "api.example.test".into(),
            path_template: "/v1/orders".into(),
            parameters: Vec::new(),
            request_schema_id: None,
            response_schema_id: None,
            status_codes: vec![200],
            status_distribution: BTreeMap::new(),
            pagination: None,
            confidence: 0.95,
            category: "api".into(),
            observed_request_ids: vec!["r1".into()],
        });

        let doc = build_openapi_document(&model);
        assert_eq!(doc.openapi, "3.1.0");
        assert!(doc.paths.contains_key("/v1/orders"));
    }
}
