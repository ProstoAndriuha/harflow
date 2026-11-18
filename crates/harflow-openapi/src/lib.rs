use std::collections::BTreeMap;

use harflow_model::{ApiModel, EndpointParameter, ParameterLocation};
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

/// Builds an OpenAPI 3.1.0 specification from discovered endpoints and their parameters.
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

            let params: Vec<Value> = ep.parameters.iter().map(parameter_to_openapi).collect();

            let mut op = serde_json::Map::new();
            op.insert("operationId".to_string(), json!(ep.operation_id));
            if !params.is_empty() {
                op.insert("parameters".to_string(), Value::Array(params));
            }
            op.insert("responses".to_string(), Value::Object(responses));

            obj.insert(method_key, Value::Object(op));
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

fn parameter_to_openapi(param: &EndpointParameter) -> Value {
    let location = match param.location {
        ParameterLocation::Path => "path",
        ParameterLocation::Query => "query",
        ParameterLocation::Header => "header",
        ParameterLocation::Cookie => "cookie",
    };

    let schema = match param.inferred_type.as_str() {
        "integer" => json!({ "type": "integer" }),
        "number" => json!({ "type": "number" }),
        "boolean" => json!({ "type": "boolean" }),
        "uuid" => json!({ "type": "string", "format": "uuid" }),
        "datetime" => json!({ "type": "string", "format": "date-time" }),
        _ => json!({ "type": "string" }),
    };

    json!({
        "name": param.name,
        "in": location,
        "required": param.required || param.location == ParameterLocation::Path,
        "schema": schema,
    })
}

#[cfg(test)]
mod tests {
    use harflow_model::Endpoint;

    use super::*;

    #[test]
    fn builds_openapi_31_document_with_paths_and_parameters() {
        let mut model = ApiModel::empty("p1", "Orders API");
        model.endpoints.push(Endpoint {
            id: "ep-1".into(),
            operation_id: Some("get_order".into()),
            method: "GET".into(),
            host: "api.example.test".into(),
            path_template: "/v1/orders/{order_id}".into(),
            parameters: vec![
                EndpointParameter {
                    name: "order_id".into(),
                    location: ParameterLocation::Path,
                    inferred_type: "integer".into(),
                    required: true,
                    observed_count: 2,
                },
                EndpointParameter {
                    name: "include".into(),
                    location: ParameterLocation::Query,
                    inferred_type: "string".into(),
                    required: false,
                    observed_count: 1,
                },
            ],
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
        let get_op = &doc.paths["/v1/orders/{order_id}"]["get"];
        assert_eq!(get_op["parameters"].as_array().unwrap().len(), 2);
        assert_eq!(get_op["parameters"][0]["schema"]["type"], "integer");
        assert_eq!(get_op["parameters"][1]["required"], false);
    }
}
