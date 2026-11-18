# HARFlow Checkpoints

## CP-001 — Phase 0 (Foundation)

- **Timestamp:** 2026-11-18T02:18:47+02:00
- **Task IDs:** `P0-01`, `P0-02`, `P0-03`, `P0-04`, `P0-05`, `P0-06`, `P0-07`, `P0-08`, `P0-09`, `P0-10`
- **Baseline Commit SHA:** `4462b5908ee523d07d3695089a6a6c2c37e23405` (`4462b59`)
- **Changed Modules:**
  - Governance & docs: `AGENTS.md`, `LICENSE`, `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md`, `docs/specifications/SRS.md`, `docs/architecture/ARCHITECTURE.md`, `docs/decisions/ADR-0001-foundation-architecture.md`, `docs/{algorithms,security,benchmarks,user-guide}/README.md`, `.agent/*`, `.github/*`
  - Schemas & fixtures: `schemas/{api-model,project,graph,analysis-report}.schema.json`, `fixtures/*` (9 synthetic captures/outputs)
  - Rust workspace: `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, 18 crates under `crates/harflow-*`
  - Frontend & shared packages: `pnpm-workspace.yaml`, `pnpm-lock.yaml`, `package.json`, `eslint.config.mjs`, `packages/{config,shared-types,ui,sdk-templates}`, `apps/web`, `apps/desktop`
- **Verified Functionality:**
  - Bounded HAR 1.2 parsing, secret redaction before persistence, query/header normalization, 13-category classification, conservative endpoint discovery, `C = 0.30V + 0.20T + 0.20U + 0.20R + 0.10S` confidence scoring with temporal gate, cycle detection, atomic project persistence with user override preservation, CLI (`doctor`, `import`, `analyze`, `endpoints`, `graph`, `generate`, `export`, `validate`, `report` with exit codes `0`/`2`/`3`/`7`), Web Studio (Dashboard, Traffic Explorer, Dependency Confirm/Reject, Dark/Light theme, EN/RU locale).
- **Tests & Checks Executed:**
  - `cargo fmt --all -- --check` -> exit `0`
  - `cargo clippy --workspace --all-targets -- -D warnings` -> exit `0` (0 warnings)
  - `cargo test --workspace` -> exit `0` (`28/28` tests passed across 18 crates, `cli_smoke`, `repo_secret_scan`, `redaction_fixture`)
  - `cargo build -p harflow-cli --release` -> exit `0`
  - `.\target\release\harflow.exe doctor --json` (5 runs) -> `34.44 ms`, `30.82 ms`, `23.57 ms`, `23.07 ms`, `22.12 ms` (`p50 = 23.57 ms`)
  - `pnpm install` -> exit `0` (`pnpm-lock.yaml` generated)
  - `pnpm lint` (`eslint .`) -> exit `0`
  - `pnpm typecheck` (`shared-types`, `ui`, `desktop`, `web`) -> exit `0`
  - `pnpm test` -> exit `0` (`5/5` Vitest tests passed across 4 test files)
  - `pnpm build` -> exit `0` (`shared-types`, `ui`, `web` Vite bundle built cleanly)
- **Known Limitations:**
  - Host Windows machine uses `stable-x86_64-pc-windows-gnu` + WinLibs MinGW-w64 UCRT (`dlltool`/`as`) rather than MSVC; native Tauri 2 desktop binary compilation and OS installers are deferred to Phase 7 per `ADR-0001`.
  - Phase 0 uses file-based atomic JSON storage (`project.json`, `api-model.json`, `sessions.json`); SQLite project indexing and streaming large-HAR parsing begin in Phase 1.
- **Recommended Next Task:** `P1-01` — Streaming HAR 1.2 Parser & Bounded Validator (`crates/harflow-har`).
