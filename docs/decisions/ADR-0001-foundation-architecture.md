# ADR-0001: Foundation Monorepo, Rust Analytical Core & Phase 0 Adapter Strategy

- **Status:** Accepted
- **Date:** 2026-11-18
- **Deciders:** HARFlow Engineering Team

## Context

HARFlow requires a cross-platform, high-throughput, local-first analysis engine capable of parsing large HAR files safely, detecting request dependencies deterministically, generating typed async SDKs (Python `httpx` and TypeScript `fetch`), and providing both a headless CLI and an interactive desktop/web UI.

We needed to finalize the Phase 0 structural choices:
1. Analytical backend language and crate decomposition.
2. Frontend workspace & shared contract architecture.
3. Windows local toolchain and Tauri 2 rollout timing.

## Decisions

1. **Rust as the Sole Analytical Backend**:
   - All parsing, sanitization, normalization, classification, endpoint discovery, schema inference, dependency detection, graph building, OpenAPI/SDK generation, mock replay, and CLI execution live in 18 focused crates under `crates/harflow-*`.
   - No Python runtime or backend service is required to run HARFlow. Python (`httpx`) is strictly a generated SDK target.

2. **Canonical Versioned `APIModel` + JSON Schemas**:
   - `schemas/api-model.schema.json`, `schemas/project.schema.json`, `schemas/graph.schema.json`, and `schemas/analysis-report.schema.json` define the canonical contracts.
   - `crates/harflow-model` and `packages/shared-types` mirror these schemas strictly with `schemaVersion = "1.0.0"`.

3. **Cargo + pnpm Monorepo**:
   - Root `Cargo.toml` manages the 18 pure-Rust crates (`resolver = "2"`).
   - `pnpm-workspace.yaml` manages `packages/*` (`config`, `shared-types`, `ui`, `sdk-templates`) and `apps/*` (`web`, `desktop`).
   - `Cargo.lock` and `pnpm-lock.yaml` are committed for reproducible builds.

4. **Phase 0 UI via `apps/web` + Typed `ApplicationAdapter`; Tauri 2 Native Build in Phase 7**:
   - `apps/web` implements the working Phase 0 developer console (Dashboard, Traffic Explorer, Dependency Inspector with Confirm/Reject, Settings, Dark/Light themes, EN/RU localization) wired through `ApplicationAdapter`.
   - `apps/desktop` provides the Phase 0 Tauri v2 capability/config scaffold (`src-tauri/tauri.conf.json`, `capabilities/default.json`) without adding heavy OS WebView/MSVC link dependencies to root `cargo test --workspace` until Phase 7.
   - Local Windows development uses `stable-x86_64-pc-windows-gnu` (self-contained `rust-mingw` linker), while `rust-toolchain.toml` remains host-triple-agnostic for Linux/macOS/Windows CI runners.

5. **Apache-2.0 Core License**:
   - Core workspace and CLI are licensed under Apache-2.0 (`LICENSE`).

## Consequences

- **Positive**: `cargo test --workspace` compiles fast in pure Rust with zero C FFI bottlenecks, headless CLI works from Day 1, and frontend components test cleanly against the typed `ApplicationAdapter` contract.
- **Negative / Trade-off**: Native desktop OS installer builds are deferred to Phase 7, and Phase 1 SQLite integration should prefer pure-Rust or bundled static crates compatible with GNU/MSVC toolchains.
