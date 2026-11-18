# HARFlow Desktop (`apps/desktop`)

Phase 0 scaffold for the Tauri 2 desktop shell.

- **Configuration:** [`src-tauri/tauri.conf.json`](./src-tauri/tauri.conf.json) enforces strict CSP (`default-src 'self'`, zero remote scripts or connections) and points `frontendDist` to `../web/dist`.
- **Capabilities:** [`src-tauri/capabilities/default.json`](./src-tauri/capabilities/default.json) follows least-privilege Tauri 2 permissions.
- **Native Rust `src-tauri` Cargo Wiring:** Scheduled for **Phase 7 (Desktop Release)** per [`docs/decisions/ADR-0001-foundation-architecture.md`](../../docs/decisions/ADR-0001-foundation-architecture.md), keeping Phase 0–6 pure-Rust core tests fast and MSVC-independent on Windows GNU hosts. In Phase 7, `src-tauri` wraps `harflow-project::dispatch_command`.
