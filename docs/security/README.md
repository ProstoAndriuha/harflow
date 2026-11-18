# HARFlow Security Architecture Notes

See root [SECURITY.md](../../SECURITY.md) for the policy and threat model summary.

## Implementation Controls

- **Harmless Placeholder Fixtures Only**: All test captures under `fixtures/` use synthetic tokens (`hf_synthetic_*`). A repository-wide automated test in `crates/harflow-security/tests/repo_secret_scan.rs` fails if common real key prefixes appear in source or fixtures.
- **Redaction Before Persistence**: `harflow-security` replaces sensitive header, cookie, query, and JSON body values with `[REDACTED:<kind>:<hash>]` fingerprints before normalization writes to disk.
- **Replay Guardrails**: `harflow-replay` rejects live execution unless explicit domain allowlists and mutating-method approvals are configured.
- **Exit Code `7`**: Any security policy violation (leakage check failure, unauthorized live replay, unapproved AI call) exits the CLI with code `7`.
