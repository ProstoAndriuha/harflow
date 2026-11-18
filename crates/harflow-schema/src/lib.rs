use std::collections::{BTreeMap, BTreeSet};

use harflow_model::{Endpoint, SchemaDefinition, SchemaField};
use harflow_normalizer::{NormalizedEntry, NormalizedSession};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const MAX_INFERENCE_DEPTH: usize = 16;

/// Canonical value types defined in SRS M06.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InferredType {
    String,
    Integer,
    Number,
    Boolean,
    Null,
    Object,
    Array,
    Datetime,
    Uuid,
    Enum,
    Union,
    Unknown,
}

impl InferredType {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Integer => "integer",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Null => "null",
            Self::Object => "object",
            Self::Array => "array",
            Self::Datetime => "datetime",
            Self::Uuid => "uuid",
            Self::Enum => "enum",
            Self::Union => "union",
            Self::Unknown => "unknown",
        }
    }
}

/// Structural difference between two inferred schema definitions (SRS M09 / M06).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SchemaDiff {
    pub added_fields: Vec<String>,
    pub removed_fields: Vec<String>,
    pub changed_fields: Vec<String>,
}

/// Infers the most specific deterministic type from a single JSON value.
#[must_use]
pub fn infer_value_type(value: &Value) -> InferredType {
    match value {
        Value::Null => InferredType::Null,
        Value::Bool(_) => InferredType::Boolean,
        Value::Number(num) => {
            if num.is_i64() || num.is_u64() {
                InferredType::Integer
            } else {
                InferredType::Number
            }
        }
        Value::Array(_) => InferredType::Array,
        Value::Object(_) => InferredType::Object,
        Value::String(s) => {
            if is_uuid(s) {
                InferredType::Uuid
            } else if is_iso_datetime(s) {
                InferredType::Datetime
            } else {
                InferredType::String
            }
        }
    }
}

/// Infers a multi-observation recursive schema definition from a slice of JSON samples.
#[must_use]
pub fn infer_schema_from_samples(
    id: impl Into<String>,
    name: impl Into<String>,
    samples: &[&Value],
) -> SchemaDefinition {
    let schema_id = id.into();
    let schema_name = name.into();
    let observation_count = samples.len();

    if samples.is_empty() {
        return SchemaDefinition {
            id: schema_id,
            name: schema_name,
            root_type: InferredType::Unknown.as_str().to_string(),
            item_type: None,
            fields: Vec::new(),
            observation_count: 0,
        };
    }

    let non_null: Vec<&Value> = samples.iter().copied().filter(|v| !v.is_null()).collect();
    if non_null.is_empty() {
        return SchemaDefinition {
            id: schema_id,
            name: schema_name,
            root_type: InferredType::Null.as_str().to_string(),
            item_type: None,
            fields: Vec::new(),
            observation_count,
        };
    }

    let obj_samples: Vec<&Map<String, Value>> =
        non_null.iter().filter_map(|v| v.as_object()).collect();
    let arr_samples: Vec<&Vec<Value>> = non_null.iter().filter_map(|v| v.as_array()).collect();

    if obj_samples.len() == non_null.len() {
        let fields = infer_object_fields(&obj_samples, 0);
        return SchemaDefinition {
            id: schema_id,
            name: schema_name,
            root_type: InferredType::Object.as_str().to_string(),
            item_type: None,
            fields,
            observation_count,
        };
    }

    if arr_samples.len() == non_null.len() {
        let all_items: Vec<&Value> = arr_samples.iter().flat_map(|a| a.iter()).collect();
        let (item_type, fields) = infer_array_shape(&all_items, 0);
        return SchemaDefinition {
            id: schema_id,
            name: schema_name,
            root_type: InferredType::Array.as_str().to_string(),
            item_type: Some(item_type),
            fields,
            observation_count,
        };
    }

    let root_types: BTreeSet<&'static str> = non_null
        .iter()
        .map(|v| infer_value_type(v).as_str())
        .collect();
    let root_type = resolve_type_set(&root_types);

    SchemaDefinition {
        id: schema_id,
        name: schema_name,
        root_type,
        item_type: None,
        fields: Vec::new(),
        observation_count,
    }
}

/// Infers request and response schemas for every discovered endpoint across normalized sessions,
/// mutating `endpoint.request_schema_id` and `endpoint.response_schema_id`.
#[must_use]
pub fn infer_endpoint_schemas(
    endpoints: &mut [Endpoint],
    sessions: &[NormalizedSession],
) -> Vec<SchemaDefinition> {
    let mut entry_index: BTreeMap<&str, &NormalizedEntry> = BTreeMap::new();
    for session in sessions {
        for entry in &session.entries {
            entry_index.insert(entry.id.as_str(), entry);
        }
    }

    let mut schemas = Vec::new();
    for ep in endpoints.iter_mut() {
        let base_name = ep
            .operation_id
            .as_deref()
            .map_or_else(|| ep.id.clone(), to_pascal_case);

        let req_samples: Vec<&Value> = ep
            .observed_request_ids
            .iter()
            .filter_map(|id| entry_index.get(id.as_str()))
            .filter_map(|entry| entry.request_body_json.as_ref())
            .collect();

        if !req_samples.is_empty() {
            let schema_id = format!("schema-{}-req", ep.id);
            let schema =
                infer_schema_from_samples(&schema_id, format!("{base_name}Request"), &req_samples);
            ep.request_schema_id = Some(schema_id);
            schemas.push(schema);
        } else {
            ep.request_schema_id = None;
        }

        let resp_samples: Vec<&Value> = ep
            .observed_request_ids
            .iter()
            .filter_map(|id| entry_index.get(id.as_str()))
            .filter_map(|entry| entry.response_body_json.as_ref())
            .collect();

        if !resp_samples.is_empty() {
            let schema_id = format!("schema-{}-resp", ep.id);
            let schema = infer_schema_from_samples(
                &schema_id,
                format!("{base_name}Response"),
                &resp_samples,
            );
            ep.response_schema_id = Some(schema_id);
            schemas.push(schema);
        } else {
            ep.response_schema_id = None;
        }
    }

    schemas
}

/// Compares two schema definitions and reports added, removed, and changed top-level fields.
#[must_use]
pub fn diff_schemas(old: &SchemaDefinition, new: &SchemaDefinition) -> SchemaDiff {
    let old_map: BTreeMap<&str, &SchemaField> =
        old.fields.iter().map(|f| (f.name.as_str(), f)).collect();
    let new_map: BTreeMap<&str, &SchemaField> =
        new.fields.iter().map(|f| (f.name.as_str(), f)).collect();

    let mut diff = SchemaDiff::default();
    for (&name, &new_field) in &new_map {
        match old_map.get(name) {
            None => diff.added_fields.push(name.to_string()),
            Some(&old_field) => {
                if old_field.inferred_type != new_field.inferred_type
                    || old_field.required != new_field.required
                    || old_field.nullable != new_field.nullable
                    || old_field.item_type != new_field.item_type
                    || old_field.union_types != new_field.union_types
                {
                    diff.changed_fields.push(name.to_string());
                }
            }
        }
    }
    for &name in old_map.keys() {
        if !new_map.contains_key(name) {
            diff.removed_fields.push(name.to_string());
        }
    }

    diff
}

fn infer_object_fields(objects: &[&Map<String, Value>], depth: usize) -> Vec<SchemaField> {
    if depth >= MAX_INFERENCE_DEPTH || objects.is_empty() {
        return Vec::new();
    }

    let parent_count = objects.len();
    let mut keys_to_values: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
    for obj in objects {
        for (k, v) in *obj {
            keys_to_values.entry(k.as_str()).or_default().push(v);
        }
    }

    keys_to_values
        .into_iter()
        .map(|(name, vals)| infer_field_from_values(name, &vals, parent_count, depth))
        .collect()
}

fn infer_field_from_values(
    name: &str,
    vals: &[&Value],
    parent_count: usize,
    depth: usize,
) -> SchemaField {
    let observed_count = vals.len();
    let required = observed_count == parent_count && parent_count > 0;
    let null_count = vals.iter().filter(|v| v.is_null()).count();
    let nullable = null_count > 0;

    let mut type_counts: BTreeMap<String, usize> = BTreeMap::new();
    for &v in vals {
        let t = infer_value_type(v).as_str().to_string();
        *type_counts.entry(t).or_insert(0) += 1;
    }

    let non_null: Vec<&Value> = vals.iter().copied().filter(|v| !v.is_null()).collect();
    if non_null.is_empty() {
        return SchemaField {
            name: name.to_string(),
            inferred_type: InferredType::Null.as_str().to_string(),
            required,
            nullable: true,
            observed_count,
            union_types: Vec::new(),
            enum_values: Vec::new(),
            item_type: None,
            nested_fields: Vec::new(),
            type_counts,
        };
    }

    let obj_refs: Vec<&Map<String, Value>> =
        non_null.iter().filter_map(|v| v.as_object()).collect();
    if obj_refs.len() == non_null.len() {
        let nested_fields = infer_object_fields(&obj_refs, depth + 1);
        return SchemaField {
            name: name.to_string(),
            inferred_type: InferredType::Object.as_str().to_string(),
            required,
            nullable,
            observed_count,
            union_types: Vec::new(),
            enum_values: Vec::new(),
            item_type: None,
            nested_fields,
            type_counts,
        };
    }

    let arr_refs: Vec<&Vec<Value>> = non_null.iter().filter_map(|v| v.as_array()).collect();
    if arr_refs.len() == non_null.len() {
        let all_items: Vec<&Value> = arr_refs.iter().flat_map(|a| a.iter()).collect();
        let (item_type, nested_fields) = infer_array_shape(&all_items, depth + 1);
        return SchemaField {
            name: name.to_string(),
            inferred_type: InferredType::Array.as_str().to_string(),
            required,
            nullable,
            observed_count,
            union_types: Vec::new(),
            enum_values: Vec::new(),
            item_type: Some(item_type),
            nested_fields,
            type_counts,
        };
    }

    let distinct_types: BTreeSet<&'static str> = non_null
        .iter()
        .map(|v| infer_value_type(v).as_str())
        .collect();
    let resolved = resolve_type_set(&distinct_types);

    if resolved == InferredType::Union.as_str() {
        let union_types = distinct_types
            .into_iter()
            .map(ToString::to_string)
            .collect();
        return SchemaField {
            name: name.to_string(),
            inferred_type: resolved,
            required,
            nullable,
            observed_count,
            union_types,
            enum_values: Vec::new(),
            item_type: None,
            nested_fields: Vec::new(),
            type_counts,
        };
    }

    if resolved == InferredType::String.as_str() {
        if let Some(enum_vals) = try_infer_enum(&non_null) {
            return SchemaField {
                name: name.to_string(),
                inferred_type: InferredType::Enum.as_str().to_string(),
                required,
                nullable,
                observed_count,
                union_types: Vec::new(),
                enum_values: enum_vals,
                item_type: None,
                nested_fields: Vec::new(),
                type_counts,
            };
        }
    }

    SchemaField {
        name: name.to_string(),
        inferred_type: resolved,
        required,
        nullable,
        observed_count,
        union_types: Vec::new(),
        enum_values: Vec::new(),
        item_type: None,
        nested_fields: Vec::new(),
        type_counts,
    }
}

fn infer_array_shape(items: &[&Value], depth: usize) -> (String, Vec<SchemaField>) {
    let non_null: Vec<&Value> = items.iter().copied().filter(|v| !v.is_null()).collect();
    if non_null.is_empty() {
        return (InferredType::Unknown.as_str().to_string(), Vec::new());
    }

    let obj_items: Vec<&Map<String, Value>> =
        non_null.iter().filter_map(|v| v.as_object()).collect();
    if obj_items.len() == non_null.len() {
        return (
            InferredType::Object.as_str().to_string(),
            infer_object_fields(&obj_items, depth),
        );
    }

    let types: BTreeSet<&'static str> = non_null
        .iter()
        .map(|v| infer_value_type(v).as_str())
        .collect();
    (resolve_type_set(&types), Vec::new())
}

fn resolve_type_set(types: &BTreeSet<&'static str>) -> String {
    if types.is_empty() {
        return InferredType::Unknown.as_str().to_string();
    }
    if types.len() == 1 {
        return (*types.iter().next().unwrap_or(&"unknown")).to_string();
    }
    if types.len() == 2 && types.contains("integer") && types.contains("number") {
        return InferredType::Number.as_str().to_string();
    }
    if types.len() == 2
        && types.contains("string")
        && (types.contains("uuid") || types.contains("datetime"))
    {
        return InferredType::String.as_str().to_string();
    }
    InferredType::Union.as_str().to_string()
}

fn try_infer_enum(non_null_strings: &[&Value]) -> Option<Vec<String>> {
    if non_null_strings.len() < 5 {
        return None;
    }

    let mut distinct = BTreeSet::new();
    for val in non_null_strings {
        let s = val.as_str()?;
        if s.is_empty() || s.len() > 32 || s.starts_with("[REDACTED:") {
            return None;
        }
        distinct.insert(s.to_string());
        if distinct.len() > 5 {
            return None;
        }
    }

    if (2..=5).contains(&distinct.len()) && distinct.len() * 2 <= non_null_strings.len() {
        Some(distinct.into_iter().collect())
    } else {
        None
    }
}

fn to_pascal_case(snake: &str) -> String {
    snake
        .split(['_', '-'])
        .filter(|s| !s.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

fn is_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts[0].len() == 8
        && parts[1].len() == 4
        && parts[2].len() == 4
        && parts[3].len() == 4
        && parts[4].len() == 12
        && parts
            .iter()
            .all(|p| p.chars().all(|c| c.is_ascii_hexdigit()))
}

fn is_iso_datetime(s: &str) -> bool {
    s.len() >= 20
        && s.as_bytes().get(4) == Some(&b'-')
        && s.as_bytes().get(7) == Some(&b'-')
        && (s.contains('T') || s.contains('t'))
        && (s.ends_with('Z') || s.contains('+'))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn infers_primitive_uuid_and_datetime_types() {
        assert_eq!(infer_value_type(&json!(42)), InferredType::Integer);
        assert_eq!(infer_value_type(&json!(3.15)), InferredType::Number);
        assert_eq!(infer_value_type(&json!(true)), InferredType::Boolean);
        assert_eq!(infer_value_type(&json!(null)), InferredType::Null);
        assert_eq!(
            infer_value_type(&json!("550e8400-e29b-41d4-a716-446655440000")),
            InferredType::Uuid
        );
        assert_eq!(
            infer_value_type(&json!("2026-11-18T00:00:00Z")),
            InferredType::Datetime
        );
    }

    #[test]
    fn infers_optional_nullable_empty_arrays_unions_enums_and_nested_objects() {
        let s1 = json!({
            "id": 1,
            "score": 10,
            "email": "a@example.test",
            "nickname": "ally",
            "status": "active",
            "tags": [],
            "legacyRef": 100,
            "profile": { "city": "Kyiv" }
        });
        let s2 = json!({
            "id": 2,
            "score": 12.5,
            "email": null,
            "nickname": "bob",
            "status": "suspended",
            "tags": [],
            "legacyRef": "old-200",
            "profile": { "city": "Lviv", "zip": "79000" }
        });
        let s3 = json!({
            "id": 3,
            "score": 9,
            "email": "c@example.test",
            "nickname": "cora",
            "status": "active",
            "tags": [],
            "legacyRef": 300,
            "profile": { "city": "Odesa" }
        });
        let s4 = json!({
            "id": 4,
            "score": 14,
            "email": "d@example.test",
            "status": "active",
            "tags": [],
            "legacyRef": "old-400",
            "profile": { "city": "Dnipro" }
        });
        let s5 = json!({
            "id": 5,
            "score": 15,
            "email": "e@example.test",
            "status": "suspended",
            "tags": [],
            "legacyRef": 500,
            "profile": { "city": "Kharkiv" }
        });

        let samples = vec![&s1, &s2, &s3, &s4, &s5];
        let schema = infer_schema_from_samples("s-user", "UserResponse", &samples);
        assert_eq!(schema.root_type, "object");
        assert_eq!(schema.observation_count, 5);

        let field = |name: &str| {
            schema
                .fields
                .iter()
                .find(|f| f.name == name)
                .unwrap_or_else(|| panic!("missing field {name}"))
        };

        // 3/5 present -> optional!
        let nickname = field("nickname");
        assert!(!nickname.required);
        assert!(!nickname.nullable);
        assert_eq!(nickname.observed_count, 3);

        // 5/5 present with 1 null -> required and nullable!
        let email = field("email");
        assert!(email.required);
        assert!(email.nullable);
        assert_eq!(email.inferred_type, "string");

        // integer + number -> widens cleanly to number
        assert_eq!(field("score").inferred_type, "number");

        // empty arrays across all 5 samples -> array of unknown
        let tags = field("tags");
        assert_eq!(tags.inferred_type, "array");
        assert_eq!(tags.item_type.as_deref(), Some("unknown"));

        // integer + string -> union preserving type_counts
        let legacy = field("legacyRef");
        assert_eq!(legacy.inferred_type, "union");
        assert_eq!(legacy.union_types, vec!["integer", "string"]);
        assert_eq!(legacy.type_counts.get("integer"), Some(&3));
        assert_eq!(legacy.type_counts.get("string"), Some(&2));

        // 5 samples of active/suspended -> conservative enum
        let status = field("status");
        assert_eq!(status.inferred_type, "enum");
        assert_eq!(status.enum_values, vec!["active", "suspended"]);

        // nested object field optionality
        let profile = field("profile");
        assert_eq!(profile.inferred_type, "object");
        let zip = profile
            .nested_fields
            .iter()
            .find(|f| f.name == "zip")
            .expect("nested zip");
        assert!(!zip.required);
        assert_eq!(zip.observed_count, 1);

        // Schema diff test
        let v2_sample = json!({
            "id": 6,
            "score": 20,
            "email": "f@example.test",
            "status": "active",
            "tags": [],
            "legacyRef": 600,
            "profile": { "city": "Kyiv" },
            "createdAt": "2026-11-18T00:00:00Z"
        });
        let schema_v2 = infer_schema_from_samples("s-user-2", "UserResponseV2", &[&v2_sample]);
        let diff = diff_schemas(&schema, &schema_v2);
        assert_eq!(diff.added_fields, vec!["createdAt"]);
        assert_eq!(diff.removed_fields, vec!["nickname"]);
        assert!(diff.changed_fields.contains(&"email".to_string()));
    }
}
