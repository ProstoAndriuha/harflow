# HARFlow Project State

**Last Updated**: 2026-11-18T03:27:00+02:00
**Current Milestone**: Phase 2 — Traffic Intelligence (`DONE`) -> Phase 3 — Dependency Engine & Session Intelligence (`READY`)
**Repository**: `https://github.com/ProstoAndriuha/harflow` (`origin/main`)

## Phase Summary

- **Phase 0 (Foundation)**: `DONE` (Checkpoint `CP-001`, commit `49d8512`, pushed to `origin/main`)
- **Phase 1 (Import & Normalization)**: `DONE` (Checkpoint `CP-002`, commit `f54c9a5`, pushed to `origin/main`)
- **Phase 2 (Traffic Intelligence)**: `DONE` (Checkpoint `CP-003`, baseline commit `5202805`, synced to `origin/main`)
- **Phase 3 (Dependency Engine & Session Intelligence)**: `READY` (`P3-01`..`P3-06` planned in `.agent/TASKS.md`)
- **Phases 4–8**: `TODO` (UI & Graph, SDK/OpenAPI Export, Mock Replay, Tauri Desktop, Optional Sanitized AI)

## Verified Baseline & Capabilities

- **Smart Traffic Cleaner (`harflow-classifier`)**: 13 traffic categories, CORS `OPTIONS` preflight detection, `CleanerConfig`, `CustomCleanerRule`, explainable rule reasons, logical `filter_sessions`, domain/request exclude & restore (`preview_cleaning`).
- **Endpoint Discovery (`harflow-endpoints`)**: Two-pass clustering (`discover_endpoints_with_overrides`), sibling-slug detection keyed with preceding segment, conservative static small-digit version rule (`/v1/`, `/api/2/`), prefixed ID detection (`ord_synthetic_*`), singularized parameter names, typed optional query inference, status code distributions, and priority pagination (`Cursor > PageNumber > OffsetLimit`).
- **Schema Inference (`harflow-schema`)**: Recursive multi-observation inference (`infer_schema_from_samples`, `infer_endpoint_schemas`, `diff_schemas`) separating absent vs `null` (`nullable`) vs optional, empty arrays (`unknown`), integer+number widening, conservative enums, union types, and nested objects.
- **Workspace & CLI Integration (`harflow-project`, `harflow-cli`, `harflow-openapi`, `schemas`, `shared-types`)**: Synchronized `api-model.schema.json` & TypeScript types, OpenAPI 3.1 `parameters` emission, `harflow clean` & `harflow schemas` subcommands, `AppCommand::{CleanerPreview, CleanerUpdate}`.
- **Phase 2 Performance Baseline (`--release`, 5 runs, `benchmarks/results/phase2-baseline.json`)**:
  - 1,000 requests full pipeline: `p50 = 201.20 ms`, `p95 = 226.51 ms` (budget `< 1,000 ms`)
  - 10,000 requests full pipeline: `p50 = 1,981.43 ms`, `p95 = 2,036.57 ms` (budget `< 5,000 ms`)
- **Verification Status**: `44` Rust workspace tests and `5` frontend Vitest tests (`4` test files) across `apps/web`, `apps/desktop`, and `packages/*`; repository secret hygiene scan active (`repo_secret_scan`).

## Open Risks & Limitations

- GitHub Actions cloud runners for `ProstoAndriuha/harflow` currently fail at job start due to remote account billing lock (`The job was not started because your account is locked due to a billing issue.`); all CI gates must be verified locally before pushing.
- Native Tauri 2 desktop bundling (`apps/desktop`) remains deferred to Phase 7 per `docs/decisions/ADR-0001-foundation-architecture.md`.

## Next Recommended Task

- **`P3-01`**: Value/cookie/header/path/query/body/pagination candidate extractor & chronological timeline builder in `crates/harflow-dependencies`.
