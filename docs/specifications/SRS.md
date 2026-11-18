# HARFlow — Software Requirements Specification (SRS)

**Document Version:** 1.0  
**Status:** Approved for implementation  
**Product Type:** Local-first Developer Tool  
**Primary Platforms:** Windows, Linux, macOS  
**UI Languages:** English (Primary), Russian (Localization)  
**Core License:** Apache-2.0  
**Architecture:** Rust Core + TypeScript Frontend + SDK Generators

---

## 1. Purpose, Audience & Principles

### 1.1. Purpose

HARFlow is a local-first tool for analyzing recorded HTTP sessions (HAR files) and automatically reconstructing typed asynchronous API SDK clients, OpenAPI specifications, and verifiable dependency graphs.

The application must enable a developer to:

1. Import one or more HAR files.
2. Inspect all recorded HTTP traffic.
3. Separate meaningful API requests from network noise.
4. Automatically discover logical endpoints and their parameters.
5. Analyze JSON payloads, forms, headers, cookies, and responses.
6. Detect dependencies between responses and subsequent requests.
7. Render and edit an interactive visual API dependency graph.
8. Identify observed authentication and session refresh mechanisms.
9. Compare multiple recorded sessions.
10. Build a single versioned intermediate `APIModel`.
11. Generate typed Python (`httpx`) and TypeScript (`fetch`) async SDKs.
12. Export an OpenAPI 3.1 specification.
13. Validate generated clients safely via mock replay (and strictly opt-in authorized live validation).
14. Persist projects and resume work seamlessly.
15. Use all core capabilities completely offline and without AI.

### 1.2. Target Audience

- Backend engineers
- API integration developers
- QA automation engineers
- DevTools engineers
- API migration and documentation engineers
- Developers of authorized integrations and parsers
- Network protocol analysts within authorized testing scopes

### 1.3. Core Principles

- **Local-first:** User data is processed locally on the device.
- **Privacy-first:** Secrets are automatically detected, masked/redacted, and excluded from logs, exports, and AI requests.
- **Deterministic-first:** Core analysis operates deterministically without LLMs.
- **Evidence-based:** Every automatically detected dependency records explicit evidence sources.
- **Human-in-the-loop:** Users can confirm, edit, or reject analyzer hypotheses without losing manual overrides on re-analysis.
- **Reproducibility:** Identical inputs and settings produce identical intermediate models and equivalent generated code.
- **Security-by-default:** Importing captures never triggers outbound requests to third-party APIs.
- **Extensibility:** New formats, analyzers, and SDK generators plug in via clean module boundaries.

### 1.4. Explicit Non-Goals

HARFlow is **not** intended for:

- Harvesting third-party credentials.
- Bypassing CAPTCHAs, anti-bot protections, or access controls.
- Breaking unknown cryptographic signatures.
- Automatically extracting inaccessible secret keys.
- Covert replay of user sessions.
- Guaranteeing full reconstruction of hidden server-side business logic from HAR captures alone.
- Executing recorded mutating requests without explicit authorization.

For unknown authentication or signing schemes, generators must emit clear extension interfaces and diagnostics rather than fabricated working implementations.

---

## 2. Functional Modules (M01–M15)

### M01 — Project Manager

- Operations: Create, Open, Rename, Delete, Import HAR, Import Multiple HAR, Recent Projects, Project Settings, Export Project, Duplicate Project, Project Statistics.
- Stored metadata: Project UUID, name, schema version, timestamps, imported sources, sanitized normalized records, analysis settings, discovered endpoints, dependencies, manual overrides, generator versions, validation results, export metadata.
- Original unredacted HAR is **not** stored inside a project by default; separate retention requires explicit user opt-in.

### M02 — HAR Import Engine

- Mandatory formats: HAR 1.2, valid HAR JSON, multi-file project import. Future: `HAR.GZ`, sanitized network JSON, Postman Collection, cURL collections.
- Pipeline: verify file & size -> validate JSON -> validate HAR structure -> detect version -> sanitize sensitive fields -> normalize -> persist clean intermediate data -> compute statistics -> pass downstream.
- Limits & safety:
  - Default file size cap: 100 MB (configurable maximum: 500 MB).
  - Protection against excessive JSON nesting depth, entry count limits, and decompression bombs.
  - Zero fetching of external URLs referenced in HAR.
  - Graceful tolerance of missing optional fields and malformed/partially recorded entries.
  - Streaming import to bound peak RAM usage.

### M03 — Traffic Normalizer

- Normalizes HTTP method, URL, host, path, query parameters, request headers, cookies, request body, response status, response headers, response cookies, response body, MIME type, timestamps, duration, initiator info (if present), and source session ID.
- Rules:
  - Preserve order and multiplicity of repeated query parameters.
  - Preserve HTTP header semantics and meaningful duplicate headers.
  - Preserve JSON array element ordering.
  - Never assume two requests are identical based solely on URL.
  - Never drop a request body on `GET` if recorded.
  - Distinguish missing fields, empty values, and `null`.
  - Track content encoding and HAR base64 encoding separately.

### M04 — Smart Traffic Cleaner

- Categories: `API`, `Authentication`, `Analytics`, `Advertising`, `Telemetry`, `Static Assets`, `Images`, `Fonts`, `CSS`, `JavaScript`, `Media`, `Documents`, `Other`.
- Signals: MIME type, URL path, domain, HTTP method, response content type, payload size, frequency, response structure, JSON presence, session behavior.
- UI actions: Keep, Exclude, Restore, Include Domain, Exclude Domain, Custom Rules, Preview Before Cleaning, Explain Classification.
- Exclusion is logical (soft filter), preserving underlying clean records for restoration.

### M05 — Endpoint Discovery Engine

- Collapses concrete requests (`GET /api/users/123`, `GET /api/users/456`) into logical endpoints (`GET /api/users/{user_id}`).
- Capabilities: endpoint grouping, dynamic path parameter inference, optional query parameter detection, parameter typing, cross-session endpoint correlation, JSON schema unioning, dominant status code detection, pagination identification, OpenAPI operation preparation.
- Guardrail: must not blindly turn static numeric segments (e.g., `/v1/`, `/v2/`) into path variables; ambiguous generalizations expose a confidence score and manual override.

### M06 — Schema Inference Engine

- Supported types: `string`, `integer`, `number`, `boolean`, `null`, `object`, `array`, `datetime`, `uuid`, `enum`, `union`, `unknown`.
- Tasks: infer request/response schemas, detect optional vs required fields (e.g., 3/5 observations is optional, not required), distinguish nullable vs optional, compare schemas across sessions, handle empty arrays, generate Pydantic and TypeScript representations, preserve observation counts when types are ambiguous.

### M07 — Dependency Detection Engine

- Supported dependency types:
  - `Response -> Request Header`
  - `Response -> Query Parameter`
  - `Response -> Request Body`
  - `Response -> URL Path`
  - `Set-Cookie -> Cookie`
  - `Pagination Chain` (cursor / next-page token)
  - `Session Chain` (`login -> authenticated request -> refresh`)
  - Transparent derived values (prefix/suffix, URL encoding/decoding, Base64 encoding/decoding, JSON field extraction) — cryptographic coincidences are not treated as proven transforms.
- Stored fields per candidate: `id`, `source_request_id`, `source_path`, `target_request_id`, `target_path`, `dependency_type`, `evidence`, `confidence`, `confirmation_status`, `transformation`, `observed_session_ids`.
- Confirmation statuses: `confirmed`, `likely`, `unknown`, `rejected`, `user_confirmed`.
- `confirmed` reflects observation under deterministic rules, not proof of internal server implementation.

### M08 — Session Intelligence

- Capabilities: Bearer Authorization detection, Cookie session detection, CSRF detection, OAuth-like flow identification, token rotation tracking, refresh endpoint detection, cross-session flow comparison, expiration detection from responses.
- Rules: Auto-refresh code generation is allowed only when sufficient observational proof and inputs exist; otherwise SDKs receive `AuthProvider`, `TokenStorage`, `RefreshStrategy`, and `AuthRequiredError` abstractions. Never embed raw HAR secrets in generated code.

### M09 — Multi-HAR Comparison

- Capabilities: compare 2+ sessions, match endpoints, detect varying vs stable parameters, corroborate request dependencies, compare auth behavior, detect schema drift and missing endpoints, merge API models.
- Reports: Session Comparison Report, Parameter Variability Report, Dependency Evidence Report, API Model Diff.
- Guardrail: never mix token/parameter values across distinct user sessions automatically.

### M10 — Graph Engine

- Views: Request Dependency Graph, Endpoint Graph, Authentication Flow, Session Timeline, Data Flow.
- Features: Zoom, Pan, Fit View, Select Node, Search Node, Filter Nodes, Highlight Path, Show Dependency Evidence, Confirm/Reject Dependency, Edit Mapping, Detect Cycles, Auto Layout, Export PNG/SVG, Export Graph JSON.
- Visual semantics: Green (`confirmed`), Yellow (`likely`), Gray (`unknown`), Red (`rejected` / conflict), Blue (`user_confirmed`). Status must always include non-color indicators (icons/text labels).

### M11 — SDK Generator

- Targets:
  1. Python Async SDK (`httpx`, Pydantic, typing, `asyncio`, `pyproject.toml`, `pytest`)
  2. TypeScript SDK (`fetch`, TypeScript types, configurable transport, `AbortSignal`, typed error handling)
  3. Future: Axios, `aiohttp`, `curl_cffi`, Go `net/http`, Kotlin Ktor
- Shared requirements: async-first, typed API, reusable HTTP client, configurable base URL and timeout, TLS verification enabled by default, typed error handling, pagination helpers, auth interfaces, safe retry policies, configurable logging, unit tests, README, usage example, compatible dependency constraints, zero embedded secrets, deterministic method names.
- Incomplete information must produce clear documented extension interfaces rather than fabricated auth/signing behavior.

### M12 — OpenAPI Generator

- Emits OpenAPI 3.1 JSON/YAML (`paths`, `operations`, `parameters`, `requestBody`, `responses`, `schemas`, `securitySchemes`, `servers`, `examples`, `tags`) with validation, deterministic operation IDs, `$ref` schemas, and missing-information warnings.

### M13 — Replay & Validation Engine

- Primary mode: **Mock Replay** against a local synthetic/sanitized mock server.
- Secondary mode: **Authorized Live Validation** (explicit user opt-in only).
- Safety controls: domain/IP allowlist, separate approval for mutating HTTP methods, dry-run preview, rate limiting, request timeouts, retry limits, isolated test credentials, redacted logs, structured report (`total`, `passed`, `failed`, `skipped`, schema mismatches, missing dependencies, auth errors, timing stats).

### M14 — Export Manager

- Formats: SDK ZIP archive, OpenAPI YAML/JSON, Graph JSON/SVG, Markdown API docs, HTML report, Sanitized HAR, HARFlow Project Bundle.
- Every export executes a final secret-leakage scan prior to writing output.

### M15 — AI Assistant (Optional)

- Capabilities: suggest SDK method names, explain request flows, explain suspected dependencies, draft endpoint descriptions, suggest test scenarios, diagnose generation errors.
- Pipeline: `Sanitized Model -> AI Context Builder -> User Approval -> LLM Provider Adapter -> Structured Response Validator -> Suggestion Review -> User Approval / Reject`.
- Constraints: disabled/optional by default; never reads raw HAR files; always previews outbound sanitized payload for user approval; zero secrets/tokens/cookies; cannot initiate replay; all graph edits require explicit confirmation.

---

## 3. UX/UI Requirements

- Modern developer tool aesthetic (VS Code DevTools / Postman caliber). Dark theme default; Light theme required. English primary UI; Russian localization required.
- Core views: Welcome Screen, Project Dashboard (7 summary cards), Traffic Explorer (virtualized table + 8-tab detail panel), Dependency Graph (left filter/endpoints, center canvas, right evidence inspector), SDK Studio, Validation Center, Settings.
- UX features: Drag-and-drop import, keyboard shortcuts, command palette, fast search, Undo/Redo on manual graph edits, progressive background loading with cancellation, accessible contrast & keyboard navigation, readable error messages.

---

## 4. Non-Functional Requirements

- **Performance:** Streaming parsing, incremental analysis, bounded memory, lazy loading, safe parallelism, graph virtualization, cancellation support.
- **Reliability:** Deterministic processing, malformed-input resilience, atomic project writes, versioned schema migrations with backups, reproducible SDK generation.
- **Security:** Offline-by-default, explicit network permissions, zero default telemetry, redacted structured logging, TLS verification enabled, no unsafe deserialization or shell execution from capture data.
- **Maintainability:** Strict module boundaries, typed contracts, comprehensive tests, ADRs, versioned schemas.
- **Compatibility:** Windows 10/11, modern Linux, macOS; Chromium and HAR 1.2 compatible Firefox captures.

---

## 5. Architecture & Intermediate `APIModel` Contract

- Single analytical backend in **Rust** (`crates/harflow-*`); **no Python backend** (Python is an SDK generation target only).
- Canonical intermediate schema defined in `schemas/api-model.schema.json` and shared TypeScript contracts in `packages/shared-types`.
- Required top-level `APIModel` fields: `schemaVersion`, `projectId`, `sources`, `endpoints`, `schemas`, `dependencies`, `authFlows`, `workflows`, `diagnostics`, `metadata`.
- Dependency Confidence Scoring starter model:

$$C = 0.30V + 0.20T + 0.20U + 0.20R + 0.10S$$

Where $V$ = value match fidelity, $T$ = temporal order validity, $U$ = value uniqueness, $R$ = cross-session repeatability, $S$ = structural/type compatibility.
Mandatory temporal preconditions still override raw weighted totals (e.g., $T \le 0 \implies$ cannot auto-confirm).
Thresholds: `0.90–1.00` (`confirmed`), `0.65–0.89` (`likely`), `0.00–0.64` (`unknown`), manual (`user_confirmed`).

---

## 6. CLI & Application API Contract

- CLI binary `harflow` shares the exact Rust Core used by desktop/application adapters.
- Subcommands: `import`, `analyze`, `endpoints`, `graph`, `generate`, `export`, `validate`, `report`, `doctor`.
- Global flags: `--json`, `--output`, `--config`, `--strict`, `--offline`, `--no-color`, `--verbose`.
- Exit codes:
  - `0`: Success
  - `1`: Runtime error
  - `2`: Invalid CLI arguments
  - `3`: Invalid source
  - `4`: Analysis error
  - `5`: Generation error
  - `6`: Validation error
  - `7`: Security policy violation
- Internal typed commands: `project.create`, `project.open`, `project.save`, `har.import`, `analysis.start`, `analysis.cancel`, `analysis.results`, `endpoints.list`, `dependencies.list`, `dependencies.confirm`, `dependencies.reject`, `graph.get`, `sdk.generate`, `sdk.preview`, `validation.mock`, `export.create`.

---

## 7. Storage, Testing, Performance Budgets & Roadmap

- **Storage:** SQLite local project index + portable versioned JSON + file storage for large safe artifacts. Secrets never stored in plaintext in SQLite, caches, or exports.
- **Performance Budgets (Release builds, warm FS, synthetic small-JSON HAR, p50/p95 across >= 5 runs):**
  - 1,000 requests: `< 1s`
  - 10,000 requests: `< 5s`, peak RAM `< 500 MB`
  - 50,000 requests: `< 20s`
  - Local CLI startup: `< 300 ms`
  - SDK generation (100 endpoints): `< 2s`
  - Golden test determinism: `100%`
  - Critical security tests: `100%` passing
- **Quality Targets (v1.0 on fixed synthetic + holdout datasets):** Confirmed dependency precision `>= 98%`, direct dependency recall `>= 85%`, zero un-evidenced confirmed edges.
- **Roadmap:** Phase 0 (Foundation) -> Phase 1 (Import & Normalization) -> Phase 2 (Traffic Intelligence) -> Phase 3 (Dependency Engine) -> Phase 4 (Professional UI) -> Phase 5 (Generators) -> Phase 6 (Validation) -> Phase 7 (Desktop Release) -> Phase 8 (Advanced Features).
