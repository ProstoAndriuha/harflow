# HARFlow Benchmarks & Performance Budgets

Performance targets defined in `docs/specifications/SRS.md` (measured on release builds, warm filesystem, synthetic small-JSON HAR captures, p50 and p95 across at least 5 runs):

| Workload | Target Budget | Measured Baseline (Phase 1) |
| -------- | ------------- | --------------------------- |
| 1,000 requests analysis | `< 1 second` | `p50 = 158.45 ms`, `p95 = 166.56 ms` (5 runs, `--release`, `benchmarks/results/phase1-baseline.json`) |
| 10,000 requests analysis | `< 5 seconds` | `p50 = 1,328.04 ms`, `p95 = 1,431.39 ms` (5 runs, `--release`, `benchmarks/results/phase1-baseline.json`) |
| 50,000 requests analysis | `< 20 seconds` | _Scheduled for Phase 2/3 load suite_ |
| Peak RAM (10k requests) | `< 500 MB` | _RSS profiling scheduled in Phase 2/8_ |
| Local CLI startup | `< 300 ms` | `p50 = 23.57 ms`, `max = 34.44 ms` (`harflow doctor --json`, 5 runs, `--release`) |
| SDK generation (100 endpoints) | `< 2 seconds` | _Scheduled for Phase 5_ |

## Running Phase 1 Benchmark

```bash
cargo run --release -p harflow-project --example phase1_bench -- --write
```

Benchmark scripts and baseline outputs live under `benchmarks/datasets/`, `benchmarks/scripts/`, and `benchmarks/results/`.
