# Changelog

All notable changes to HARFlow are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Phase 1 (Import & Normalization):
  - Streaming HAR 1.2 and `.har.gz` parser (`harflow-har`) with `BoundedReader` size/nesting limits, gzip decompression-bomb protection, `_initiator` extraction, and partial-entry recovery.
  - Expanded credential sanitizer (`harflow-security`) covering `JWT`, `Basic`/`Digest`, `application/x-www-form-urlencoded`, and fingerprint-correlated `Cookie` / `Set-Cookie` redaction with non-secret pagination allowlists.
  - Full Traffic Normalizer (`harflow-normalizer`) tracking `FieldPresence` (`Missing`/`Null`/`Value`), request form parameters, base64-encoded JSON response decoding/sanitization, and separate `Content-Encoding` vs HAR encoding metadata.
  - Local SQLite 11-table project index (`harflow-project`) with `PRAGMA user_version` forward migrations, pre-migration backups (`harflow.sqlite.bak.v<N>`), `analysis_runs` history, and backward-compatible Phase 0 project backfill.
  - CLI (`harflow-cli`) streaming multi-HAR and `.har.gz` import, `--config` import limits, and `--strict` partial-entry rejection.
  - Deterministic property/fuzz-seed invariants (`harflow-normalizer`) and 1k/10k release benchmark suite (`phase1_bench`).
- Phase 0 (Foundation) monorepo scaffold, canonical specification (`SRS.md`), agent workflow governance (`AGENTS.md`, `.agent/*`), JSON Schemas (`schemas/*`), 18-crate pure-Rust workspace (`crates/*`), working `harflow` CLI, synthetic fixtures (`fixtures/*`), `packages/*`, working React + TypeScript web UI (`apps/web`), desktop scaffold (`apps/desktop`), and GitHub CI workflows.
