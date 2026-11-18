# Contributing to HARFlow

Thank you for contributing to HARFlow! Please read `AGENTS.md`, `docs/specifications/SRS.md`, and `docs/architecture/ARCHITECTURE.md` before proposing changes.

## Engineering Rules

1. **Keep Rust Core decoupled**: Core crates (`crates/harflow-*`) must never import React, Tauri, or UI-specific crates.
2. **Never commit secrets or real captures**: All files under `fixtures/` and tests must contain only synthetic placeholder tokens (`hf_synthetic_*`).
3. **Preserve backward compatibility**: Follow `schemas/api-model.schema.json` and `schemas/project.schema.json` versioning rules; unknown fields must not drop supported data.
4. **Deterministic behavior first**: Analysis algorithms must produce identical outputs for identical inputs.
5. **Document architecture decisions**: Significant technical choices require an ADR under `docs/decisions/`.

## Quality Gates

Before opening a pull request, run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

## Commit Style

Use descriptive Conventional Commits:

- `feat(har): add streaming depth-bounded HAR parser`
- `fix(security): redact nested bearer tokens in form payloads`
- `docs(srs): clarify multi-session isolation rules`
- `test(cli): verify exit code 3 on truncated HAR input`
