# HARFlow

**Local-first HTTP traffic analyzer, API structure reconstructor, evidence-backed dependency graph builder, and typed async SDK generator.**

HARFlow analyzes recorded HAR 1.2 network captures offline, strips and redacts sensitive credentials before storage, discovers logical REST API endpoints and schemas, detects request-to-request dependencies with explicit evidence, and emits typed Python (`httpx`) / TypeScript (`fetch`) SDK clients and OpenAPI 3.1 specifications.

---

## Core Principles

- **Local-first**: All parsing, analysis, and storage run locally on your machine with zero network dependency.
- **Privacy-first**: Secrets (`Authorization`, `Cookie`, `Set-Cookie`, tokens, passwords) are detected and redacted before indexing, storage, logging, or export.
- **Deterministic-first**: Endpoint discovery, schema inference, and dependency detection operate deterministically without LLM calls.
- **Evidence-based**: Every inferred dependency records its `ruleId`, source/target requests, confidence score, and human confirmation status (`confirmed`, `likely`, `unknown`, `rejected`, `user_confirmed`).
- **Human-in-the-loop**: Users inspect evidence and confirm, reject, or override analyzer suggestions without losing overrides on re-analysis.
- **Security-by-default**: Importing or analyzing HAR files never replays requests to external services.

---

## Monorepo Architecture

```text
harflow/
├── crates/       # Pure-Rust analytical core, project store, generators, and CLI
├── apps/
│   ├── web/      # React + TypeScript UI communicating via typed ApplicationAdapter
│   └── desktop/  # Tauri 2 desktop wrapper scaffold (full native bundling in Phase 7)
├── packages/
│   ├── config/        # Shared TypeScript configurations
│   ├── shared-types/  # Canonical TypeScript APIModel & command contracts
│   ├── ui/            # Accessible design-system primitives (StatusBadge, Card, Button)
│   └── sdk-templates/ # Python httpx & TypeScript fetch SDK templates
├── schemas/      # Canonical JSON Schemas (api-model, project, graph, analysis-report)
├── fixtures/     # Synthetic-only test fixtures (no real credentials)
├── tests/        # Cross-layer integration, security, golden, e2e, and perf suites
├── benchmarks/   # Performance datasets, scripts, and p50/p95 records
├── docs/         # SRS, architecture docs, ADRs, algorithm & security guides
└── .agent/       # Engineering state, backlog, checkpoints, quality gates, logs
```

---

## Quick Start

### Prerequisites

- **Rust** (stable toolchain with `rustfmt` and `clippy`)
- **Node.js** 22+ / **pnpm** 10+

### Rust Core & CLI Verification

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Run the CLI against a synthetic fixture:

```bash
cargo run -p harflow-cli -- doctor --json
cargo run -p harflow-cli -- import fixtures/har-valid/minimal-rest.har.json --project ./tmp-demo
cargo run -p harflow-cli -- analyze ./tmp-demo
cargo run -p harflow-cli -- endpoints ./tmp-demo --json
cargo run -p harflow-cli -- graph ./tmp-demo --format json
cargo run -p harflow-cli -- report ./tmp-demo --format markdown
```

### Frontend Workspace Verification

```bash
pnpm install
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

---

## CLI Exit Codes

| Code | Meaning |
| ---- | ------- |
| `0`  | Success |
| `1`  | Runtime / I/O error |
| `2`  | Invalid CLI arguments |
| `3`  | Invalid or malformed HAR source |
| `4`  | Analysis failure |
| `5`  | Code / specification generation failure |
| `6`  | Validation / replay failure |
| `7`  | Security policy violation |

---

## Development Roadmap

- **Phase 0 — Foundation**: Monorepo, Rust workspace, React UI, schemas, CI, `.agent/` governance *(Current)*
- **Phase 1 — Import & Normalization**: Streaming HAR 1.2 parser, secret sanitizer, normalizer, SQLite/JSON project store
- **Phase 2 — Traffic Intelligence**: Traffic cleaner, endpoint discovery, schema inference
- **Phase 3 — Dependency Engine**: Evidence-backed dependency graph, confidence calibration, multi-HAR comparison
- **Phase 4 — Professional UI**: Dashboard, virtualized Traffic Explorer, interactive React Flow graph, inspector
- **Phase 5 — Generators**: Python `httpx` async SDK, TypeScript `fetch` SDK, OpenAPI 3.1, ZIP export
- **Phase 6 — Validation**: Mock replay harness, golden tests, validation reports
- **Phase 7 — Desktop Release**: Tauri 2 desktop builds, installers, security audit
- **Phase 8 — Advanced Features**: Optional sanitized-context AI assistant, advanced session intelligence

---

## License

Core workspace is licensed under the [Apache License 2.0](./LICENSE).
