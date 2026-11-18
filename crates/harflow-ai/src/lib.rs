use harflow_core::HarflowError;
use harflow_model::ApiModel;
use harflow_security::contains_unredacted_secret_marker;
use serde::{Deserialize, Serialize};

/// Explicit opt-in policy required before any sanitized model context leaves the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiPolicy {
    pub enabled: bool,
    pub user_approved_payload: bool,
}

/// Builds a sanitized summary context from `ApiModel` only after explicit enablement and user approval.
pub fn build_sanitized_prompt_preview(
    model: &ApiModel,
    policy: &AiPolicy,
) -> Result<String, HarflowError> {
    if !policy.enabled {
        return Err(HarflowError::SecurityViolation(
            "AI assistant is disabled in local-only mode".to_string(),
        ));
    }
    if !policy.user_approved_payload {
        return Err(HarflowError::SecurityViolation(
            "outbound AI prompt requires explicit user payload approval".to_string(),
        ));
    }

    let endpoint_lines: Vec<String> = model
        .endpoints
        .iter()
        .map(|e| {
            format!(
                "- {} {} (category: {})",
                e.method, e.path_template, e.category
            )
        })
        .collect();

    let preview = format!(
        "Project: {}\nEndpoints ({}):\n{}",
        model.metadata.project_name,
        model.endpoints.len(),
        endpoint_lines.join("\n")
    );

    if contains_unredacted_secret_marker(&preview) {
        return Err(HarflowError::SecurityViolation(
            "AI context preview failed secret-hygiene check".to_string(),
        ));
    }

    Ok(preview)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_explicit_enablement_and_user_approval() {
        let model = ApiModel::empty("p1", "Demo");
        let disabled = AiPolicy::default();
        assert_eq!(
            build_sanitized_prompt_preview(&model, &disabled)
                .unwrap_err()
                .exit_code()
                .as_i32(),
            7
        );

        let unapproved = AiPolicy {
            enabled: true,
            user_approved_payload: false,
        };
        assert_eq!(
            build_sanitized_prompt_preview(&model, &unapproved)
                .unwrap_err()
                .exit_code()
                .as_i32(),
            7
        );

        let approved = AiPolicy {
            enabled: true,
            user_approved_payload: true,
        };
        let preview = build_sanitized_prompt_preview(&model, &approved).expect("preview");
        assert!(preview.contains("Project: Demo"));
    }
}
