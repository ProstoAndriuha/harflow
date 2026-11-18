use std::fs;
use std::path::PathBuf;

use harflow_model::PaginationStyle;
use harflow_project::ProjectWorkspace;
use uuid::Uuid;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates dir")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

#[test]
fn golden_mixed_traffic_filters_noise_clusters_endpoints_infers_schemas_and_restores() {
    let dir = std::env::temp_dir().join(format!("harflow_p2_golden_{}", Uuid::new_v4()));
    let fixture = repo_root().join("fixtures/har-valid/ecommerce-mixed-traffic.har.json");
    let raw_bytes = fs::read(&fixture).expect("read mixed traffic fixture");

    let mut ws = ProjectWorkspace::create(&dir, "Ecommerce Intelligence").expect("create");
    ws.import_har_bytes("ecommerce-mixed-traffic.har.json", &raw_bytes)
        .expect("import");
    ws.analyze().expect("analyze");

    // 4 noise requests excluded (JS, PNG, analytics, CORS OPTIONS), 5 API/Auth kept; all 9 raw entries preserved.
    assert_eq!(ws.model.metadata.total_requests, 9);
    assert_eq!(ws.model.metadata.excluded_requests, 4);
    assert_eq!(ws.sessions[0].entries.len(), 9);

    // Zero raw synthetic token leaked anywhere in api-model.json.
    let api_model_disk = fs::read_to_string(dir.join("api-model.json")).expect("read api-model");
    assert!(!api_model_disk.contains("hf_synthetic_ecom_tok_1"));

    // Verify discovered endpoints & pagination.
    let users_ep = ws
        .model
        .endpoints
        .iter()
        .find(|e| e.path_template == "/v1/users/{user_id}")
        .expect("users endpoint");
    assert_eq!(users_ep.observed_request_ids.len(), 3);
    assert_eq!(users_ep.parameters[0].inferred_type, "integer");
    let users_schema_id = users_ep
        .response_schema_id
        .as_ref()
        .expect("users response schema");

    let products_ep = ws
        .model
        .endpoints
        .iter()
        .find(|e| e.path_template == "/v1/products")
        .expect("products endpoint");
    let pagination = products_ep.pagination.as_ref().expect("cursor pagination");
    assert_eq!(pagination.style, PaginationStyle::Cursor);
    assert_eq!(
        pagination.response_next_field.as_deref(),
        Some("nextCursor")
    );

    // Verify inferred schema rules on /v1/users/{user_id}.
    let users_schema = ws
        .model
        .schemas
        .iter()
        .find(|s| &s.id == users_schema_id)
        .expect("find users schema");
    assert_eq!(users_schema.observation_count, 3);

    let field = |name: &str| {
        users_schema
            .fields
            .iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("missing field {name}"))
    };
    assert!(field("email").required && !field("email").nullable);
    assert!(field("bio").required && field("bio").nullable);
    assert!(!field("nickname").required);
    assert_eq!(field("tags").item_type.as_deref(), Some("unknown"));
    assert_eq!(field("legacyRef").inferred_type, "union");
    assert_eq!(field("legacyRef").union_types, vec!["integer", "string"]);
    assert_eq!(field("profile").inferred_type, "object");
    let tier = field("profile")
        .nested_fields
        .iter()
        .find(|f| f.name == "tier")
        .expect("nested tier");
    assert!(!tier.required);

    // Determinism: re-running analyze() yields identical endpoints and schemas.
    let endpoints_before = ws.model.endpoints.clone();
    let schemas_before = ws.model.schemas.clone();
    ws.analyze().expect("re-analyze");
    assert_eq!(ws.model.endpoints, endpoints_before);
    assert_eq!(ws.model.schemas, schemas_before);

    // Logical restore without re-importing HAR.
    let preview = ws
        .restore_request("sess-0001-req-0001")
        .expect("restore request 1");
    assert_eq!(preview.excluded_entries, 3);
    ws.analyze().expect("analyze after restore");
    assert_eq!(ws.model.metadata.excluded_requests, 3);

    let _ = fs::remove_dir_all(&dir);
}
