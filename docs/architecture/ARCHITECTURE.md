# HARFlow Architecture

## Overview

HARFlow uses a **pure-Rust analytical core** compiled into modular workspace crates (`crates/harflow-*`), consumed identically by the headless CLI (`harflow-cli`) and graphical clients (`apps/web` in Phase 0, and `apps/desktop` via Tauri 2 IPC in Phase 7).

```text
+-------------------------------------------------------------------+
|                   Presentation Layer (TypeScript)                 |
|        apps/web (React UI)         apps/desktop (Tauri 2)         |
+---------------------------------+---------------------------------+
                                  |
                  Typed ApplicationAdapter Commands
                (packages/shared-types <-> JSON IPC)
                                  |
+---------------------------------v---------------------------------+
|                    Application & CLI Adapters                     |
|         harflow-cli              harflow-project (Service)        |
+---------------------------------+---------------------------------+
                                  |
+---------------------------------v---------------------------------+
|                 Generators, Validation & Export                   |
|  harflow-generator   harflow-openapi   harflow-replay   -report   |
+---------------------------------+---------------------------------+
                                  |
                    Canonical APIModel (v1.0.0)
                 schemas/api-model.schema.json
                                  |
+---------------------------------v---------------------------------+
|                   Analytical Intelligence Layer                   |
|  harflow-endpoints  harflow-schema  harflow-dependencies  -graph  |
|  harflow-sessions   harflow-classifier                            |
+---------------------------------+---------------------------------+
                                  |
+---------------------------------v---------------------------------+
|             Ingestion, Normalization & Security Gate              |
|     harflow-har      harflow-normalizer     harflow-security      |
+---------------------------------+---------------------------------+
                                  |
+---------------------------------v---------------------------------+
|                       Foundation Contracts                        |
|           harflow-core              harflow-model                 |
+-------------------------------------------------------------------+
```

## Module Dependency Rules

1. **Zero UI Coupling in Core**: No crate under `crates/` may depend on React, Vite, Tauri, or browser APIs.
2. **No Python Analytical Backend**: Python is strictly an output target of `harflow-generator`.
3. **Single Source of Truth Model**: Every downstream generator (`harflow-generator`, `harflow-openapi`, `harflow-replay`, `harflow-report`, `harflow-ai`) consumes `harflow-model::ApiModel` (aligned with `schemas/api-model.schema.json`).
4. **Security-by-Construction Boundary**: All HAR bytes pass through `harflow-har` (bounded parser) and `harflow-normalizer` + `harflow-security` (redaction) before entering project storage or analytical indices.
5. **Typed Command Boundary**: Frontend and backend exchange explicit typed commands (`project.create`, `har.import`, `analysis.start`, `dependencies.confirm`, `dependencies.reject`, etc.) defined in `packages/shared-types` and `crates/harflow-project`.

## Storage Model

- **Portable Project Directory**:
  - `project.json` (`schemas/project.schema.json`) — UUID, schema version, settings, statistics, generator metadata.
  - `api-model.json` (`schemas/api-model.schema.json`) — sanitized model, endpoints, dependencies, evidence, user overrides.
  - Atomic writes (`<file>.tmp` -> rename) prevent partial corruption on crash.
- **SQLite Index (Phase 1+)**: Local indexing and run history without storing unredacted credentials.
- **Raw HAR Retention**: Disabled by default (`retainOriginalHar: false`).

## Security Boundary

- Imported HARs, headers, bodies, project files, and AI responses are treated as **untrusted input**.
- Offline-by-default (`offlineOnly: true`): zero outbound network connections during import, normalization, analysis, or generation.
- `harflow-security::RedactedValue` prevents accidental secret leakage in `Debug`, `Display`, structured logs, and serialized artifacts.
