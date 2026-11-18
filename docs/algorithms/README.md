# HARFlow Algorithms Reference

This directory documents deterministic algorithms implemented across HARFlow analytical crates:

1. **Smart Traffic Classification (`harflow-classifier`)**: Rule-based categorization into 13 traffic categories (`API`, `Authentication`, `Analytics`, `Advertising`, `Telemetry`, `Static Assets`, `Images`, `Fonts`, `CSS`, `JavaScript`, `Media`, `Documents`, `Other`) with CORS `OPTIONS` preflight detection, custom `CleanerConfig` / `CustomCleanerRule` support, logical filtering preserving raw normalized entries, and explainable rule reasons.
2. **Endpoint Discovery (`harflow-endpoints`)**: Two-pass path clustering (`discover_endpoints_with_overrides`) preserving static small-digit version segments (e.g. `/v1/`, `/api/2/`), detecting UUID, numeric, prefixed-token, and high-cardinality sibling slugs keyed with their preceding path segment, inferring singularized parameter names, optional/typed query parameters, status code distributions, and priority pagination style (`Cursor > PageNumber > OffsetLimit`) with confidence scores.
3. **Schema Inference (`harflow-schema`)**: Recursive multi-observation JSON schema inference (`infer_schema_from_samples`, `infer_endpoint_schemas`, `diff_schemas`) distinguishing absent vs `null` (`nullable`) vs optional fields, empty arrays (`unknown`), integer+number widening, conservative finite enums, union types, nested objects, and schema drift diffs.
4. **Dependency Detection (`harflow-dependencies`)**:
   - Confidence scoring starter formula:
     $$C = 0.30V + 0.20T + 0.20U + 0.20R + 0.10S$$
   - Mandatory temporal ordering gate ($T > 0$) before any dependency can be marked `confirmed` or `likely`.
   - Thresholds: `>= 0.90` (`confirmed`), `0.65–0.89` (`likely`), `< 0.65` (`unknown`).
5. **Graph Cycle Analysis (`harflow-graph`)**: Directed cycle detection distinguishing logical pagination/refresh loops from invalid chronological dependencies.
