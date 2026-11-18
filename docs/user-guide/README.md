# HARFlow User Guide

## CLI Quick Reference

```bash
# Check local environment & capabilities
harflow doctor --json

# Import a HAR 1.2 or .har.gz capture into a local project (redacts secrets automatically)
harflow import ./capture.har --project ./my-project

# Run deterministic analysis
harflow analyze ./my-project

# Inspect cleaner rules, endpoints, schemas, dependencies & sessions
harflow clean preview ./my-project --json
harflow endpoints ./my-project --json
harflow schemas ./my-project --json
harflow dependencies ./my-project --json
harflow sessions ./my-project --json
harflow graph ./my-project --kind endpoint --format json

# Export OpenAPI 3.1 or Markdown report
harflow export ./my-project --format openapi --output ./out
harflow report ./my-project --format markdown --output ./out
```

## Web Console (`apps/web`)

Start the frontend development server:

```bash
pnpm --filter @harflow/web dev
```

### Studio Views

1. **Dashboard**: 7 project summary cards, multi-file `.har` / `.json` / `.har.gz` drag-and-drop import (local decompression with size bounds), synthetic demo loader, and imported session summary.
2. **Traffic Explorer**: Window-virtualized table for 10k+ entries, category/method/status/domain/visibility filters, **Smart Traffic Cleaner** (13 categories, custom rules, decision preview diff, soft exclude/restore without losing raw entries), and an **8-tab Request/Response Inspector** (`Overview`, `Headers`, `Query`, `Cookies`, `Body`, `Response`, `Timing`, `Security`).
3. **Endpoints & Schemas**: Clustered endpoint browser, path template overrides (with Undo/Redo), pagination & status distributions, recursive `SchemaTreeView` (`required`/`optional`, `nullable`, `union`, `enum`), plus TypeScript and Python Pydantic previews.
4. **Dependency Graph**: 3-pane studio supporting all **5 graph perspectives** (`Endpoint Graph`, `Request Dependency`, `Auth Flow`, `Session Timeline`, `Data Flow`), zoom/pan/fit/reset, path highlighting, cycle banners, edge/confidence filters, provenance evidence inspector, manual confirm/reject/mapping edits with Undo/Redo, and SVG/JSON export.
5. **Sessions & Diff**: Authentication lifecycle (`Bearer`, `CookieSession`, `Csrf`, `ApiKey`, `OauthLike`), redacted token rotations, `401`/`403` expiration challenges, `autoRefreshEligible` readiness, and Multi-HAR parameter variability / schema drift diffs.
6. **Settings**: Dark / Light theme toggle, EN / RU localization, adapter mode indicator, and keyboard shortcut reference.

### Keyboard Shortcuts

- `Ctrl+K` / `Cmd+K` — Open Command Palette
- `Ctrl+Z` / `Cmd+Z` — Undo dependency, cleaner, or endpoint template edit
- `Ctrl+Y` / `Cmd+Shift+Z` — Redo
- `Alt+1`..`Alt+6` — Jump between Studio tabs
