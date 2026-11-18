use serde::{Deserialize, Serialize};
use serde_json::Value;

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
}
