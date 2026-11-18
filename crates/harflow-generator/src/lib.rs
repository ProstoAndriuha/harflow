use harflow_core::HarflowError;
use harflow_model::ApiModel;
use harflow_security::contains_unredacted_secret_marker;
use serde::{Deserialize, Serialize};

/// Supported SDK generation targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SdkTarget {
    PythonHttpx,
    TypeScriptFetch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SdkBundle {
    pub target: SdkTarget,
    pub files: Vec<GeneratedFile>,
}

/// Validates that generated SDK source files contain zero leaked raw secret markers.
pub fn validate_bundle_no_secrets(bundle: &SdkBundle) -> Result<(), HarflowError> {
    for file in &bundle.files {
        if contains_unredacted_secret_marker(&file.content) {
            return Err(HarflowError::SecurityViolation(format!(
                "generated SDK file '{}' contains an unredacted secret marker",
                file.path
            )));
        }
    }
    Ok(())
}

/// Generates a deterministic Phase 0 SDK preview bundle with zero embedded credentials.
pub fn generate_sdk_preview(
    model: &ApiModel,
    target: SdkTarget,
) -> Result<SdkBundle, HarflowError> {
    let files = match target {
        SdkTarget::PythonHttpx => vec![
            GeneratedFile {
                path: "README.md".to_string(),
                content: format!(
                    "# {} Python Async SDK (Preview)\n\nEndpoints discovered: {}\nCredentials are supplied at runtime via `AuthProvider`.\n",
                    model.metadata.project_name,
                    model.endpoints.len()
                ),
            },
            GeneratedFile {
                path: "harflow_sdk/client.py".to_string(),
                content: render_python_preview(model),
            },
        ],
        SdkTarget::TypeScriptFetch => vec![
            GeneratedFile {
                path: "README.md".to_string(),
                content: format!(
                    "# {} TypeScript Fetch SDK (Preview)\n\nEndpoints discovered: {}\nCredentials are supplied at runtime via `AuthProvider`.\n",
                    model.metadata.project_name,
                    model.endpoints.len()
                ),
            },
            GeneratedFile {
                path: "src/client.ts".to_string(),
                content: render_typescript_preview(model),
            },
        ],
    };

    let bundle = SdkBundle { target, files };
    validate_bundle_no_secrets(&bundle)?;
    Ok(bundle)
}

fn render_python_preview(model: &ApiModel) -> String {
    let mut lines = vec![
        "from typing import Protocol".to_string(),
        "import httpx".to_string(),
        String::new(),
        "class AuthProvider(Protocol):".to_string(),
        "    async def get_headers(self) -> dict[str, str]: ...".to_string(),
        String::new(),
        "class HarflowClient:".to_string(),
        "    def __init__(self, base_url: str, auth: AuthProvider | None = None, timeout: float = 30.0) -> None:".to_string(),
        "        self._client = httpx.AsyncClient(base_url=base_url, timeout=timeout, verify=True)".to_string(),
        "        self._auth = auth".to_string(),
    ];

    for ep in &model.endpoints {
        let name = ep
            .operation_id
            .clone()
            .unwrap_or_else(|| format!("{}_endpoint", ep.method.to_ascii_lowercase()));
        lines.push(String::new());
        lines.push(format!("    async def {name}(self) -> httpx.Response:"));
        lines.push(
            "        headers = await self._auth.get_headers() if self._auth else {}".to_string(),
        );
        lines.push(format!(
            "        return await self._client.request(\"{}\", \"{}\", headers=headers)",
            ep.method, ep.path_template
        ));
    }

    lines.join("\n") + "\n"
}

fn render_typescript_preview(model: &ApiModel) -> String {
    let mut lines = vec![
        "export interface AuthProvider {".to_string(),
        "  getHeaders(): Promise<Record<string, string>>;".to_string(),
        "}".to_string(),
        String::new(),
        "export class HarflowClient {".to_string(),
        "  constructor(private readonly baseUrl: string, private readonly auth?: AuthProvider) {}"
            .to_string(),
    ];

    for ep in &model.endpoints {
        let name = ep
            .operation_id
            .clone()
            .unwrap_or_else(|| format!("{}_endpoint", ep.method.to_ascii_lowercase()));
        lines.push(String::new());
        lines.push(format!(
            "  async {name}(signal?: AbortSignal): Promise<Response> {{"
        ));
        lines
            .push("    const headers = this.auth ? await this.auth.getHeaders() : {};".to_string());
        lines.push(format!(
            "    return fetch(`${{this.baseUrl}}{}`, {{ method: '{}', headers, signal }});",
            ep.path_template, ep.method
        ));
        lines.push("  }".to_string());
    }

    lines.push("}".to_string());
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bundle_with_unredacted_secret_header() {
        let bundle = SdkBundle {
            target: SdkTarget::PythonHttpx,
            files: vec![GeneratedFile {
                path: "bad.py".into(),
                content: "Authorization: Bearer hf_synthetic_unredacted_secret_value".into(),
            }],
        };
        let err = validate_bundle_no_secrets(&bundle).unwrap_err();
        assert_eq!(err.exit_code().as_i32(), 7);
    }

    #[test]
    fn generates_python_and_ts_previews_cleanly() {
        let model = ApiModel::empty("p1", "Demo");
        let py = generate_sdk_preview(&model, SdkTarget::PythonHttpx).expect("py preview");
        let ts = generate_sdk_preview(&model, SdkTarget::TypeScriptFetch).expect("ts preview");
        assert!(py.files.iter().any(|f| f.path.ends_with("client.py")));
        assert!(ts.files.iter().any(|f| f.path.ends_with("client.ts")));
    }
}
