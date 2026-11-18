# HARFlow Project State

**Last Updated:** 2026-11-18
**Repository:** `https://github.com/ProstoAndriuha/harflow` (`public`, branch `main`)
**Current Phase:** Phase 1 — Import & Normalization (`DONE`)
**Next Phase:** Phase 2 — Traffic Intelligence (`READY`)
**Toolchain:**
- Rust: `1.100.0` (Windows host GNU toolchain `x86_64-pc-windows-gnu` + WinLibs MinGW-w64 `2.47.20260726` binutils / GCC `16.2.0`; cross-platform `rust-toolchain.toml` pinned to `1.100.0`)
- Node.js: `v24.17.0`
- pnpm: `11.8.0`
- Python: `3.14.6`

## Module Readiness Matrix

| Layer / Module | Location | Current Status | Notes |
| -------------- | -------- | -------------- | ----- |
| Governance & SRS | `AGENTS.md`, `docs/specifications/SRS.md` | DONE | Approved spec, ADR-0001, security, benchmarks & contributing docs |
| Canonical Schemas | `schemas/*.schema.json` | DONE | Draft 2020-12 contracts (`api-model`, `project`, `graph`, `analysis-report`) |
| Rust Core Workspace | `crates/harflow-*` (18 crates) | DONE | Pure-Rust, zero clippy warnings, `38/38` tests passing |
| Streaming HAR & Security | `harflow-har`, `harflow-security`, `harflow-normalizer` | DONE | Streaming `.har`/`.har.gz`, bomb protection, JWT/Basic/form/cookie redaction, `FieldPresence`, 250-iter property tests |
| SQLite Project Index | `crates/harflow-project/src/sqlite_store.rs` | DONE | All 11 SRS tables, `PRAGMA user_version` migrations with pre-migration backups, Phase 0 `sessions.json` backfill |
| Headless CLI | `crates/harflow-cli` | DONE | Streaming multi-HAR & `.har.gz` import, `--config`, `--strict`, exit codes `0`/`2`/`3`, `p50 = 23.57 ms` |
| Synthetic Fixtures | `fixtures/*` | DONE | 9 synthetic fixtures (`hf_synthetic_*`), `repo_secret_scan` passes |
| Shared TS Packages | `packages/{config,shared-types,ui,sdk-templates}` | DONE | Strict TS, accessible `StatusBadge`, Vitest passing |
| Web Console | `apps/web` | DONE | Dashboard, Traffic Explorer, Dependency Confirm/Reject, EN/RU, Dark/Light |
| Desktop Scaffold | `apps/desktop` | DONE | Tauri 2 config/capabilities + bridge descriptor tested (native bundling in Phase 7) |
| CI Workflows | `.github/workflows/*.yml` | DONE | 7 GitHub Actions workflows configured |

## Active Blockers

- None locally (note: GitHub Actions cloud runners remain gated by account billing lock on GitHub).

## Next Recommended Task

- **`P2-01`**: Smart Traffic Cleaner rules & logical exclude/restore (`crates/harflow-classifier`) — deterministic rules across all 13 categories, domain include/exclude, custom rules, and soft-exclusion with restore.
