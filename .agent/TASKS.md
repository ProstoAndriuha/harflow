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
| `P1-05` | P1 | CLI `import` & `analyze` production pipeline + `--strict`/`--offline` enforcement | `P1-04` | IN_PROGRESS | Multi-HAR import, statistics calculation, strict exit codes |
| `P1-06` | P1 | Phase 1 property, fuzz-seed & 1k/10k HAR benchmark tests | `P1-05` | TODO | Property tests for normalizer invariants + initial benchmark telemetry |

---

## Future Epics (Phases 2–8)

- `P2-01`: Phase 2 — Smart Traffic Cleaner, Endpoint Discovery clustering, Schema Inference (`harflow-classifier`, `harflow-endpoints`, `harflow-schema`)
- `P3-01`: Phase 3 — Full Dependency Detection Engine, calibration benchmarks, Multi-HAR comparison & Session Intelligence (`harflow-dependencies`, `harflow-sessions`)
- `P4-01`: Phase 4 — Full React Flow Dependency Graph, Virtualized Traffic Explorer, 8-tab Inspector, Command Palette, Undo/Redo (`apps/web`, `packages/ui`)
- `P5-01`: Phase 5 — Python `httpx` + TypeScript `fetch` SDK generators, OpenAPI 3.1 generator, Export ZIP (`harflow-generator`, `harflow-openapi`)
- `P6-01`: Phase 6 — Mock Replay server, Validation Center, Golden SDK tests (`harflow-replay`)
- `P7-01`: Phase 7 — Native Tauri 2 desktop bundling, installers, security hardening (`apps/desktop`)
- `P8-01`: Phase 8 — Optional sanitized-context AI assistant & extended generators (`harflow-ai`)
