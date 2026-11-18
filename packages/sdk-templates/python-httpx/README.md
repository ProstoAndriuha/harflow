# Python `httpx` Async SDK Template

Target stack for Phase 5 `harflow-generator`:

- `httpx.AsyncClient` with `verify=True` (TLS enabled by default)
- Pydantic v2 models
- Explicit `AuthProvider` / `TokenStorage` interfaces (no embedded credentials)
- `pytest` unit test suite & `pyproject.toml`
