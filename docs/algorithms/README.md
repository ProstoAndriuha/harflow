# HARFlow Algorithms Reference

This directory documents deterministic algorithms implemented across HARFlow analytical crates:

1. **Smart Traffic Classification (`harflow-classifier`)**: Rule-based categorization into 13 traffic categories (`API`, `Authentication`, `Analytics`, `Advertising`, `Telemetry`, `Static Assets`, `Images`, `Fonts`, `CSS`, `JavaScript`, `Media`, `Documents`, `Other`) with explainable rule reasons.
2. **Endpoint Discovery (`harflow-endpoints`)**: Path normalization and conservative dynamic-parameter clustering preserving static version routes (`/v1/`, `/v2/`) and attaching confidence scores.
3. **Schema Inference (`harflow-schema`)**: Multi-observation JSON typing (`string`, `integer`, `number`, `boolean`, `null`, `object`, `array`, `datetime`, `uuid`, `enum`, `union`, `unknown`) distinguishing absent vs `null` vs optional fields.
4. **Dependency Detection (`harflow-dependencies`)**:
   - Confidence scoring starter formula:
     $$C = 0.30V + 0.20T + 0.20U + 0.20R + 0.10S$$
   - Mandatory temporal ordering gate ($T > 0$) before any dependency can be marked `confirmed` or `likely`.
   - Thresholds: `>= 0.90` (`confirmed`), `0.65–0.89` (`likely`), `< 0.65` (`unknown`).
5. **Graph Cycle Analysis (`harflow-graph`)**: Directed cycle detection distinguishing logical pagination/refresh loops from invalid chronological dependencies.
