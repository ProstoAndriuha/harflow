# HARFlow Quality Gates

Every phase and task must pass the applicable checks below before being marked `DONE`.

## 1. Rust Core & CLI Gates

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## 2. Frontend & Shared Packages Gates

```bash
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

## 3. Security & Secret Hygiene Gates

- `cargo test -p harflow-security --all-targets` passes (including `repo_secret_scan`).
- Zero real credentials or unredacted sensitive tokens anywhere in `fixtures/`, `crates/`, `apps/`, `packages/`, or logs.
- No outbound HTTP requests executed during import, normalization, analysis, or SDK generation.

## 4. Phase 0 CLI Smoke & Startup Gates

- `harflow doctor --json` exits `0` and completes within `< 300 ms` startup budget.
- `harflow import` / `analyze` / `endpoints` / `graph` / `export` / `report` work end-to-end on synthetic fixtures.
- Malformed HAR source exits with code `3` (`InvalidSource`).
- Invalid CLI flags exit with code `2` (`InvalidArgs`).
