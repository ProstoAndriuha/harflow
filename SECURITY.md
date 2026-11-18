# HARFlow Security Policy

HAR files frequently contain live `Authorization` headers, session cookies, CSRF tokens, personal data, and sensitive API payloads. Security and privacy are foundational architectural requirements in HARFlow.

## Core Security Invariants

1. **Offline-by-default**: Zero outbound network requests during HAR import, normalization, classification, endpoint discovery, dependency detection, or SDK generation.
2. **No automatic replay of imported traffic**: HARFlow never executes recorded HTTP requests automatically.
3. **Automatic redaction before persistence**: Raw credentials (`Authorization`, `Cookie`, `Set-Cookie`, `X-API-Key`, token/password keys) are replaced with deterministic non-reversible fingerprints (`[REDACTED:<kind>:<hash>]`) before project storage, logs, or exports.
4. **No embedded secrets in generated SDKs**: Generated Python and TypeScript clients always use explicit credential provider interfaces or environment variable injection—never literal tokens from HAR captures.
5. **Strict bounded parsing**: HAR imports enforce configurable file size limits (100 MB default, 500 MB max), JSON nesting depth caps, entry limits, and malformed/truncated input handling without panics.
6. **Allowlisted validation only**: Live validation is disabled by default and requires explicit user domain allowlists, separate confirmation for mutating HTTP methods (`POST`, `PUT`, `PATCH`, `DELETE`), TLS verification, and SSRF protections.
7. **AI context isolation**: Optional AI features are disabled by default, never receive raw HAR files or secrets, and require explicit user preview and approval before sending sanitized summaries.

## Threat Model Coverage

| Threat | Mitigation |
| ------ | ---------- |
| Credential leakage | Secret detection (`harflow-security`), redaction wrapper, export scan |
| Malicious / hostile HAR | Strict bounds (`ImportLimits`), depth validation, zero code execution |
| HTML / script injection | Escaping in UI & reports, strict Tauri CSP |
| Unsafe replay / SSRF | Mock-first replay, domain allowlists, explicit mutating-method approval |
| AI data leakage | Sanitized `APIModel` context only + mandatory user approval gate |
| Supply-chain compromise | Committed `Cargo.lock` & `pnpm-lock.yaml`, security CI workflows |

## Reporting a Vulnerability

Please **do not** open a public GitHub issue with real credentials or exploitable details. Report security issues privately to the project maintainers with reproduction steps and affected versions/commits.
