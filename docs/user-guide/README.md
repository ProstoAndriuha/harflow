# HARFlow User Guide

## CLI Quick Reference

```bash
# Check local environment & capabilities
harflow doctor --json

# Import a HAR 1.2 capture into a local project (redacts secrets automatically)
harflow import ./capture.har --project ./my-project

# Run deterministic analysis
harflow analyze ./my-project

# Inspect discovered endpoints & dependency graph
harflow endpoints ./my-project --json
harflow graph ./my-project --format json

# Export OpenAPI 3.1 or Markdown report
harflow export ./my-project --format openapi --output ./out
harflow report ./my-project --format markdown --output ./out
```

## Web Console (`apps/web`)

Start the frontend development server:

```bash
pnpm --filter @harflow/web dev
```

Use the top bar to switch **Dark / Light** theme, **EN / RU** localization, import local `.har` files offline, inspect sanitized traffic, and confirm or reject detected dependencies.
