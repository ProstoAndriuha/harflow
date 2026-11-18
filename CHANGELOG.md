# Changelog

All notable changes to HARFlow are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Phase 3 (Dependency Engine & Session Intelligence):
  - Credential-payload redacted fingerprint correlation (`parse_redacted_marker`, `fingerprint_secret` in `harflow-security`) matching JSON token fields (`access_token`) with `Authorization: Bearer ...` headers without persisting raw secrets.
  - Linear-time $O(N)$ Dependency Detection Engine (`harflow-dependencies`) with chronological per-session timelines, 8-location producer/consumer extraction, `Identity`/`PrefixSuffix`/`UrlEncoding`/`Base64`/`JsonExtract` matching, pre-existing client & weak-scalar exclusion filters, and calibrated $C = 0.30V + 0.20T + 0.20U + 0.20R + 0.10S$ confidence scoring.
  - Full 5-perspective `GraphViewModel` builder (`build_graph`) and cycle contradiction analyzer (`analyze_graph_cycles` in `harflow-graph`) distinguishing pagination self-loops from request-level contradictions, plus synced `schemas/graph.schema.json` and `@harflow/shared-types`.
  - Session Intelligence & Multi-HAR comparison (`harflow-sessions`) covering `Bearer`, `CookieSession`, `Csrf`, `ApiKey`, `OauthLike`, signature hooks, redacted token rotations, expiration signals, parameter variability (`Constant`/`SessionScoped`/`RequestVolatile`), and schema drift diffing.
  - Workspace workflow synthesis, manual override retention across re-analysis, CLI commands (`harflow dependencies`, `harflow sessions`, `harflow graph --kind`), curated primary & unseen holdout precision/recall/F1 suites (`confirmedPrecision = 1.0`, `directRecall = 1.0`, `f1 = 1.0`), and Phase 3 1k/10k release baseline (`benchmarks/results/phase3-baseline.json`).
- Phase 2 (Traffic Intelligence):
  - 13-category rule-based traffic classifier (`harflow-classifier`) with CORS `OPTIONS` preflight detection, `CleanerConfig` / `CustomCleanerRule` support, explainable rule reasons, logical session filtering (`filter_sessions`), and preview/exclusion/restore overrides.
  - Two-pass Endpoint Discovery engine (`harflow-endpoints`) with sibling-slug clustering keyed by preceding segment, static small-digit version preservation (`/v1/`, `/api/2/`), prefixed ID detection, singularized parameter naming, typed optional query inference, status distributions, and priority pagination detection (`Cursor > PageNumber > OffsetLimit`).
  - Recursive multi-observation JSON Schema inference (`harflow-schema`) distinguishing absent vs `null` vs optional fields, empty arrays (`unknown`), integer+number widening, conservative enums, union types, nested objects, and schema drift diffing (`diff_schemas`).
  - Workspace, OpenAPI 3.1, schema/shared-types, and CLI integration (`harflow clean`, `harflow schemas`, `CleanerPreview`, `CleanerUpdate`), plus synthetic mixed-traffic golden suite (`ecommerce-mixed-traffic.har.json`) and Phase 2 1k/10k release benchmark (`benchmarks/results/phase2-baseline.json`).
- Phase 1 (Import & Normalization):
  - Streaming HAR 1.2 and `.har.gz` parser (`harflow-har`) with `BoundedReader` size/nesting limits, gzip decompression-bomb protection, `_initiator` extraction, and partial-entry recovery.
  - Expanded credential sanitizer (`harflow-security`) covering `JWT`, `Basic`/`Digest`, `application/x-www-form-urlencoded`, and fingerprint-correlated `Cookie` / `Set-Cookie` redaction with non-secret pagination allowlists.
  - Full Traffic Normalizer (`harflow-normalizer`) tracking `FieldPresence` (`Missing`/`Null`/`Value`), request form parameters, base64-encoded JSON response decoding/sanitization, and separate `Content-Encoding` vs HAR encoding metadata.
  - Local SQLite 11-table project index (`harflow-project`) with `PRAGMA user_version` forward migrations, pre-migration backups (`harflow.sqlite.bak.v<N>`), `analysis_runs` history, and backward-compatible Phase 0 project backfill.
  - CLI (`harflow-cli`) streaming multi-HAR and `.har.gz` import, `--config` import limits, and `--strict` partial-entry rejection.
  - Deterministic property/fuzz-seed invariants (`harflow-normalizer`) and 1k/10k release benchmark suite (`phase1_bench`).
- Phase 0 (Foundation) monorepo scaffold, canonical specification (`SRS.md`), agent workflow governance (`AGENTS.md`, `.agent/*`), JSON Schemas (`schemas/*`), 18-crate pure-Rust workspace (`crates/*`), working `harflow` CLI, synthetic fixtures (`fixtures/*`), `packages/*`, working React + TypeScript web UI (`apps/web`), desktop scaffold (`apps/desktop`), and GitHub CI workflows.
