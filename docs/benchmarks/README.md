# HARFlow Benchmarks & Performance Budgets

Performance targets defined in `docs/specifications/SRS.md` (measured on release builds, warm filesystem, synthetic small-JSON HAR captures, p50 and p95 across at least 5 runs):

| Workload | Target Budget |
| -------- | ------------- |
| 1,000 requests analysis | `< 1 second` |
| 10,000 requests analysis | `< 5 seconds` |
| 50,000 requests analysis | `< 20 seconds` |
| Peak RAM (10k requests) | `< 500 MB` |
| Local CLI startup | `< 300 ms` |
| SDK generation (100 endpoints) | `< 2 seconds` |

Benchmark scripts and baseline outputs live under `benchmarks/datasets/`, `benchmarks/scripts/`, and `benchmarks/results/`.
