# HARFlow Project State

**Last Updated**: 2026-11-18T05:02:00+02:00
**Current Milestone**: Phase 4 — Professional UI (`DONE`) -> Phase 5 — SDK & OpenAPI Generators (`READY`)
**Repository**: `https://github.com/ProstoAndriuha/harflow` (`origin/main`)

## Phase Summary

- **Phase 0 (Foundation)**: `DONE` (Checkpoint `CP-001`, commit `49d8512`, pushed to `origin/main`)
- **Phase 1 (Import & Normalization)**: `DONE` (Checkpoint `CP-002`, commit `f54c9a5`, pushed to `origin/main`)
- **Phase 2 (Traffic Intelligence)**: `DONE` (Checkpoint `CP-003`, commit `5202805` / `4d902f9`, pushed to `origin/main`)
- **Phase 3 (Dependency Engine & Session Intelligence)**: `DONE` (Checkpoint `CP-004`, baseline commit `8111785` / `9037376`, synced to `origin/main`)
- **Phase 4 (Professional UI)**: `DONE` (Checkpoint `CP-005`, baseline commit `e4bf2f1`, synced to `origin/main`)
- **Phase 5 (SDK & OpenAPI Generators)**: `READY` (`P5-01`..`P5-06` planned in `.agent/TASKS.md`)
- **Phases 6–8**: `TODO` (Mock Replay, Tauri Desktop, Optional Sanitized AI)

## Verified Baseline & Capabilities

- **Professional Web Studio (`apps/web`, `packages/ui`, `packages/shared-types`)**:
  - Client-side Phase 2/3 analytical parity in `InMemoryApplicationAdapter` preserving the 16-command desktop contract (`apps/desktop` bridge verified).
  - Bounded Undo/Redo history stack (`historyStack.ts`) with toolbar buttons and `Ctrl+Z`/`Ctrl+Y`/`Ctrl+Shift+Z` shortcuts for dependency edits, cleaner updates, and endpoint template overrides.
  - Windowed `VirtualTable` (`10k` rows), multi-filter Traffic Explorer, Smart Cleaner bar (13 categories, custom rules, preview diff, soft exclude/restore), and accessible 8-tab `RequestInspector`.
  - Interactive `DependencyGraphCanvas` + 3-pane `DependencyGraphPage` (all 5 perspectives, zoom/pan/fit/reset, path highlight, cycle warning, non-color badges, evidence/mapping inspector, SVG/JSON export).
  - `EndpointsSchemasPage` (recursive `SchemaTreeView`, TS/Pydantic previews, template overrides) and `SessionsComparisonPage` (M08 token rotations & expiration signals, M09 parameter variability & schema drift).
  - Global `CommandPalette` (`Ctrl+K`), `Alt+1..6` shortcuts, multi-file `.har`/`.har.gz` `HarDropzone` (`DecompressionStream` with byte limit guard), 100% EN/RU dictionary parity, skip link, and Dark/Light themes.
- **Rust Core & Intelligence Baseline**:
  - Primary (`29` edges) & Holdout (`10` edges) dependency evaluation: `confirmedPrecision = 1.0`, `directRecall = 1.0`, `f1 = 1.0`, `unEvidencedConfirmed = 0`.
  - 1,000 requests pipeline: `p50 = 169.67 ms`, `p95 = 198.45 ms` (`< 1,000 ms`); 10,000 requests pipeline: `p50 = 1,650.61 ms`, `p95 = 2,309.88 ms` (`< 5,000 ms`).
- **Verification Status**: `58/58` Rust workspace tests (`cargo test --workspace` including `repo_secret_scan` and `precision_recall_eval`) and `19/19` Vitest tests (`11` test files across `shared-types`, `desktop`, `ui`, `web`), `eslint .`, `tsc`, and `vite build` (`375.43 kB` JS / `3.89 kB` CSS) all passing. Real-world HAR testing verified across `DummyJSON`, `Hacker News`, and `PokéAPI` (`BUG-002`..`BUG-005` fixed and verified).

## Open Risks & Limitations

- GitHub Actions cloud runners for `ProstoAndriuha/harflow` currently fail at job start due to remote account billing lock (`The job was not started because your account is locked due to a billing issue.`); all CI gates must be verified locally before pushing.
- Native Tauri 2 desktop bundling (`apps/desktop`) remains deferred to Phase 7 per `docs/decisions/ADR-0001-foundation-architecture.md`.

## Next Recommended Task

- **`P5-01`**: OpenAPI 3.1 generator (`paths`, `parameters`, `requestBody`, `responses`, `$ref` schemas, `securitySchemes`, diagnostics) (`crates/harflow-openapi`).
