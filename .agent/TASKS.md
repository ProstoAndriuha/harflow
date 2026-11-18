# HARFlow Task Backlog

Task statuses: `TODO` | `IN_PROGRESS` | `BLOCKED` | `IN_REVIEW` | `DONE`  
Priorities: `P0` (Critical) | `P1` (Core) | `P2` (Important) | `P3` (Enhancement)

---

## Phase 0 — Foundation

| ID | Priority | Title | Dependencies | Status | Acceptance Criteria |
| -- | -------- | ----- | ------------ | ------ | ------------------- |
| `P0-01` | P0 | Git repo, toolchain pin, root docs & license | None | DONE | `.git`, `rust-toolchain.toml` (`1.100.0`), `.editorconfig`, `.gitattributes`, `.gitignore`, `LICENSE`, `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md` created and verified |
| `P0-02` | P0 | Canonical `AGENTS.md`, `SRS.md`, `ARCHITECTURE.md`, `ADR-0001` & `.agent/` files | `P0-01` | DONE | All governance, spec, ADR, and `.agent/` tracking files created and aligned with approved SRS |
| `P0-03` | P0 | Canonical Draft 2020-12 JSON Schemas (`schemas/*.schema.json`) | `P0-02` | DONE | `api-model`, `project`, `graph`, `analysis-report` schemas cover all required fields |
| `P0-04` | P0 | 18-crate pure-Rust workspace (`crates/harflow-*`) & unit tests | `P0-03` | DONE | Strict crate DAG, `ExitCode`, `ApiModel`, redaction, confidence formula, atomic project storage verified (`25` unit/integration crate tests + `3` CLI tests = `28` passed) |
| `P0-05` | P0 | Headless `harflow` CLI binary & exit-code smoke tests | `P0-04` | DONE | `doctor`, `import`, `analyze`, `endpoints`, `graph`, `generate`, `export`, `validate`, `report` + exit codes `0`, `2`, `3` tested; release startup `p50 = 23.57 ms` |
| `P0-06` | P0 | Synthetic fixtures (`fixtures/*`) & automated repo secret hygiene scan | `P0-04` | DONE | 9 synthetic fixtures (`hf_synthetic_*`) + `repo_secret_scan` & `redaction_fixture` passing |
| `P0-07` | P0 | pnpm workspace, `packages/{config,shared-types,ui,sdk-templates}` | `P0-03` | DONE | Shared TS types validate `minimal-api-model.json`, `StatusBadge` has icon + label + color, 3/3 package tests pass |
| `P0-08` | P0 | Working `apps/web` console & `apps/desktop` Phase 0 Tauri v2 scaffold | `P0-07` | DONE | Dashboard, Traffic Explorer, Dependency Confirm/Reject, EN/RU, Dark/Light themes, Vitest (`2/2` app + desktop tests) & Vite build passing |
| `P0-09` | P0 | 7 GitHub Actions CI workflows & issue/PR templates | `P0-05`, `P0-08` | DONE | `rust.yml`, `frontend.yml`, `integration.yml`, `security.yml`, `sdk-generation.yml`, `benchmark.yml`, `release.yml` created |
| `P0-10` | P0 | Run all Phase 0 quality gates, record Checkpoint `CP-001` & clean Git commit | `P0-01`..`P0-09` | DONE | `cargo fmt/clippy/test`, `cargo build -p harflow-cli --release`, `pnpm lint/typecheck/test/build` all exited `0` |

---

## Phase 1 — Import & Normalization (Ready)

| ID | Priority | Title | Dependencies | Status | Acceptance Criteria |
| -- | -------- | ----- | ------------ | ------ | ------------------- |
| `P1-01` | P1 | Streaming HAR 1.2 parser with size/depth/entry bounds (`harflow-har`) | `P0-10` | DONE | Streaming parser handles 100 MB default / 500 MB max captures within bounded RAM, tolerates missing optional fields & partial entries |
| `P1-02` | P1 | Comprehensive secret & PII detection/redaction rules (`harflow-security`) | `P0-10` | DONE | Detects headers, cookies, JWT-like tokens, form fields, query params; zero raw secrets in logs/storage |
| `P1-03` | P1 | Full Traffic Normalizer (`harflow-normalizer`) | `P1-01`, `P1-02` | DONE | Preserves duplicate headers/query order, GET bodies, absent vs empty vs null, base64/content encodings |
| `P1-04` | P1 | SQLite local project index + migrations + portable JSON export (`harflow-project`) | `P1-03` | DONE | SQLite tables (`projects`, `sources`, `requests`, `endpoints`, `schemas`, `dependencies`, `evidence`, `user_overrides`, `analysis_runs`, `validation_runs`, `generated_artifacts`) + migration backups |
| `P1-05` | P1 | CLI `import` & `analyze` production pipeline + `--strict`/`--offline` enforcement | `P1-04` | DONE | Multi-HAR import, statistics calculation, strict exit codes |
| `P1-06` | P1 | Phase 1 property, fuzz-seed & 1k/10k HAR benchmark tests | `P1-05` | DONE | Property tests for normalizer invariants + initial benchmark telemetry (`1k p50 = 158.45 ms`, `10k p50 = 1328.04 ms`) |

---

## Phase 2 — Traffic Intelligence (Ready)

| ID | Priority | Title | Dependencies | Status | Acceptance Criteria |
| -- | -------- | ----- | ------------ | ------ | ------------------- |
| `P2-01` | P1 | Smart Traffic Cleaner rules & logical exclude/restore (`harflow-classifier`) | `P1-06` | DONE | Deterministic rules across all 13 categories, domain include/exclude, custom rules, soft-exclusion preserving underlying entries, explainable reason & preview diff |
| `P2-02` | P1 | Endpoint Discovery multi-segment clustering & pagination detection (`harflow-endpoints`) | `P2-01` | DONE | Groups variable path IDs/UUIDs/slugs while preserving static route/version segments, infers optional query params, status code distributions & pagination parameters with confidence scores |
| `P2-03` | P1 | Multi-observation Schema Inference Engine (`harflow-schema`) | `P2-02` | DONE | Infers request/response JSON schemas across multiple samples, distinguishes nullable vs optional (e.g. 3/5 present -> optional), handles empty arrays, enums, unions & unknown types |
| `P2-04` | P1 | ProjectWorkspace & CLI integration for cleaning rules, endpoints & schemas (`harflow-project`, `harflow-cli`) | `P2-03` | DONE | Persists cleaned flags, discovered endpoints & inferred schemas in SQLite/APIModel; adds cleaner preview/exclude/restore & schema inspection CLI commands |
| `P2-05` | P1 | Phase 2 unit, golden APIModel & 10k intelligence benchmark tests | `P2-04` | DONE | Golden REST API fixtures produce deterministic `APIModel` endpoints & schemas; 10k intelligence pipeline completes in `p50 = 1,981.43 ms` (`< 5s` budget) |

---

## Active Epic: Phase 3 — Dependency Engine & Session Intelligence

| ID | Priority | Task | Depends On | Status | Acceptance Criteria |
| -- | -------- | ---- | ---------- | ------ | ------------------- |
| `P3-01` | P1 | Value/cookie/header/path/query/body/pagination candidate extractor & timeline builder (`harflow-dependencies`) | `P2-05` | DONE | Indexes chronological producer responses & consumer inputs across path, query, headers, cookies, JSON/form bodies & pagination tokens with deterministic provenance locators |
| `P3-02` | P1 | Transformation evaluation, weak-match exclusion, weighted confidence calibration ($C = 0.30V+0.20T+0.20U+0.20R+0.10S$) & contradiction/cycle guardrails (`harflow-dependencies`, `harflow-graph`) | `P3-01` | DONE | Evaluates direct, trimmed, URL/base64-decoded & JSON-escaped matches; excludes weak structural coincidences; blocks non-chronological links & distinguishes valid pagination loops from contradictions |
| `P3-03` | P1 | Multi-HAR comparison & Session Intelligence (`harflow-sessions`) | `P3-02` | DONE | Compares volatile vs static parameters across sessions, detects Bearer/Cookie/CSRF/API-key/OAuth2/refresh flows without exposing raw secrets |
| `P3-04` | P1 | `ProjectWorkspace`, `GraphViewModel` & CLI integration for dependencies/sessions (`harflow-project`, `harflow-graph`, `harflow-cli`, `schemas`, `shared-types`) | `P3-03` | DONE | Persists dependencies, evidence links & session diff findings in SQLite/APIModel; exposes `harflow dependencies` / `harflow sessions` CLI commands & synced TypeScript/JSON schemas |
| `P3-05` | P1 | Labeled synthetic dataset precision/recall/F1 evaluation harness (`harflow-dependencies`) | `P3-04` | DONE | Measures deterministic Precision >= 98%, Recall >= 85% and documented F1 across curated multi-flow datasets |
| `P3-06` | P1 | Holdout validation, 10k dependency benchmark & Checkpoint `CP-004` | `P3-05` | DONE | Primary & holdout both achieve `confirmedPrecision = 1.0`, `directRecall = 1.0`, `f1 = 1.0`, `unEvidencedConfirmed = 0`; 10k pipeline completes in `p50 = 1,650.61 ms` (`< 5s` budget) |

---

## Active Epic: Phase 4 — Professional UI (`apps/web`, `packages/ui`, `packages/shared-types`)

| ID | Priority | Task | Depends On | Status | Acceptance Criteria |
| -- | -------- | ---- | ---------- | ------ | ------------------- |
| `P4-01` | P1 | Application state store, undo/redo history stack & `InMemoryApplicationAdapter` parity with Phase 2/3 cleaner, schemas, graph views & session intelligence (`apps/web`, `packages/shared-types`) | `P3-06` | DONE | Typed UI store manages clean/restore, endpoint overrides, graph confirm/reject undo/redo stack, schema inspection & session comparison state without breaking 16-command desktop contract |
| `P4-02` | P1 | Virtualized Traffic Explorer table, search/filter bar, Smart Cleaner controls & 8-tab Request/Response Inspector (`apps/web`, `packages/ui`) | `P4-01` | DONE | Renders 10k+ sanitized entries smoothly; supports category/domain/status search, cleaner preview/exclude/restore, and 8-tab inspector (`Overview`, `Headers`, `Query`, `Cookies`, `Body`, `Response`, `Timing`, `Security`) |
| `P4-03` | P1 | Interactive Dependency Graph Studio (`apps/web`, `packages/ui`) | `P4-01` | DONE | Supports 5 graph perspectives, zoom/pan/fit/search, node/edge filtering, evidence inspector, manual confirm/reject/edit with Undo/Redo & SVG/JSON export + non-color status badges |
| `P4-04` | P1 | Endpoints, Schemas & Multi-HAR Session Comparison views (`apps/web`) | `P4-01` | DONE | Displays clustered endpoints, recursive schema fields (`required`/`nullable`/`union`/`enum`), session lifecycle rotations & Multi-HAR parameter variability / schema drift reports |
| `P4-05` | P2 | Command Palette (`Ctrl+K`), keyboard shortcuts, drag-and-drop multi-HAR import, full EN/RU localization & Light/Dark accessibility polish (`apps/web`, `packages/ui`) | `P4-02`, `P4-03`, `P4-04` | DONE | Complete EN/RU i18n coverage, keyboard navigation & accessible contrast across Dark/Light themes, multi-file `.har`/`.har.gz` dropzone |
| `P4-06` | P1 | Phase 4 Vitest/Testing-Library component & accessibility suite, bundle build check & Checkpoint `CP-005` | `P4-05` | IN_PROGRESS | Component/a11y/undo-redo tests, lint, typecheck, Vite build & `CP-005` synced to `origin/main` |

---

## Future Epics (Phases 5–8)

- `P5-01`: Phase 5 — Python `httpx` + TypeScript `fetch` SDK generators, OpenAPI 3.1 generator, Export ZIP (`harflow-generator`, `harflow-openapi`)
- `P6-01`: Phase 6 — Mock Replay server, Validation Center, Golden SDK tests (`harflow-replay`)
- `P7-01`: Phase 7 — Native Tauri 2 desktop bundling, installers, security hardening (`apps/desktop`)
- `P8-01`: Phase 8 — Optional sanitized-context AI assistant & extended generators (`harflow-ai`)
