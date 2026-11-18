# HARFlow Project State

**Last Updated**: 2026-11-18T04:26:00+02:00
**Current Milestone**: Phase 3 — Dependency Engine & Session Intelligence (`DONE`) -> Phase 4 — Professional UI (`READY`)
**Repository**: `https://github.com/ProstoAndriuha/harflow` (`origin/main`)

## Phase Summary

- **Phase 0 (Foundation)**: `DONE` (Checkpoint `CP-001`, commit `49d8512`, pushed to `origin/main`)
- **Phase 1 (Import & Normalization)**: `DONE` (Checkpoint `CP-002`, commit `f54c9a5`, pushed to `origin/main`)
- **Phase 2 (Traffic Intelligence)**: `DONE` (Checkpoint `CP-003`, commit `5202805` / `4d902f9`, pushed to `origin/main`)
- **Phase 3 (Dependency Engine & Session Intelligence)**: `DONE` (Tasks `P3-01`..`P3-06`, pending Checkpoint `CP-004` & push)
- **Phase 4 (Professional UI)**: `READY` (`P4-01`..`P4-06` planned in `.agent/TASKS.md`)
- **Phases 5–8**: `TODO` (SDK/OpenAPI Export, Mock Replay, Tauri Desktop, Optional Sanitized AI)

## Verified Baseline & Capabilities

- **Dependency Detection (`harflow-dependencies`, `harflow-security`)**: Credential-payload fingerprint correlation (`parse_redacted_marker`, `fingerprint_secret`), chronological per-session timelines, 8-location extractors, linear $O(N)$ forward hash index supporting `Identity`, `PrefixSuffix`, `UrlEncoding`, `Base64`, and `JsonExtract`, weak-scalar/pre-existing-client filters, calibrated $C = 0.30V + 0.20T + 0.20U + 0.20R + 0.10S$ confidence scoring, and cross-session corroboration/contradiction handling.
- **Graph Engine (`harflow-graph`, `schemas/graph.schema.json`, `shared-types`)**: All 5 M10 graph perspectives (`build_graph`) and request-vs-endpoint cycle contradiction peeling (`analyze_graph_cycles`).
- **Session Intelligence & Multi-HAR Comparison (`harflow-sessions`)**: `Bearer`, `CookieSession`, `Csrf`, `ApiKey`, `OauthLike`, and custom signature detection; redacted token rotations, expiration signals, strict `auto_refresh_eligible`; Multi-HAR parameter variability (`Constant`, `SessionScoped`, `RequestVolatile`), auth differences, and schema drift reports.
- **Workspace & CLI Integration (`harflow-project`, `harflow-cli`)**: End-to-end `analyze()` dependency detection, workflow synthesis, cycle contradiction diagnostics, manual override preservation, `harflow dependencies`, `harflow sessions`, and `harflow graph --kind`.
- **Phase 3 Accuracy & Performance (`--release`, `benchmarks/results/phase3-dependency-eval.json`, `benchmarks/results/phase3-baseline.json`)**:
  - Primary Dataset (`29` ground-truth edges): `confirmedPrecision = 1.0`, `directRecall = 1.0`, `f1 = 1.0`, `unEvidencedConfirmed = 0`
  - Unseen Holdout Dataset (`10` ground-truth edges): `confirmedPrecision = 1.0`, `directRecall = 1.0`, `f1 = 1.0`, `unEvidencedConfirmed = 0`
  - 1,000 requests full pipeline (5 runs): `p50 = 169.67 ms`, `p95 = 198.45 ms` (SRS budget `< 1,000 ms`)
  - 10,000 requests full pipeline (5 runs): `p50 = 1,650.61 ms`, `p95 = 2,309.88 ms` (SRS budget `< 5,000 ms`)
- **Verification Status**: `54/54` Rust workspace tests and `5` frontend Vitest tests (`4` test files) across `apps/web`, `apps/desktop`, and `packages/*` passing; repository secret hygiene scan active (`repo_secret_scan`).

## Open Risks & Limitations

- GitHub Actions cloud runners for `ProstoAndriuha/harflow` currently fail at job start due to remote account billing lock (`The job was not started because your account is locked due to a billing issue.`); all CI gates must be verified locally before pushing.
- Native Tauri 2 desktop bundling (`apps/desktop`) remains deferred to Phase 7 per `docs/decisions/ADR-0001-foundation-architecture.md`.

## Next Recommended Task

- **`P4-01`**: Application state store, undo/redo history stack & `InMemoryApplicationAdapter` parity with Phase 2/3 cleaner, schemas, graph views & session intelligence (`apps/web`, `packages/shared-types`).
