# HARFlow Bug Tracker

Severity levels: `P0` (Critical) | `P1` (Core) | `P2` (Important) | `P3` (Minor)  
Statuses: `OPEN` | `IN_PROGRESS` | `FIXED` | `VERIFIED`

| ID | Severity | Component | Summary | Status | Regression Test | Notes |
| -- | -------- | --------- | ------- | ------ | --------------- | ----- |
| `BUG-001` | `P2` | `harflow-dependencies` | Short 4-char pure-alpha config constants (`env: prod`) with matching key names scored `0.91` (`Confirmed`) in single-session detection | `VERIFIED` | `holdout_dataset_meets_precision_recall_and_evidence_targets` | Fixed generally by requiring explicit ID/token key alignment on short pure-alpha strings and scaling string uniqueness penalties for non-ID alphabetical words |
