# HARFlow Benchmarks & Performance Budgets

Performance targets defined in `docs/specifications/SRS.md` (measured on release builds, warm filesystem, synthetic small-JSON HAR captures, p50 and p95 across at least 5 runs):

| Workload | Target Budget | Measured Baseline (Phase 1) | Measured Baseline (Phase 2) | Measured Baseline (Phase 3) |
| -------- | ------------- | --------------------------- | --------------------------- | --------------------------- |
| 1,000 requests analysis | `< 1 second` | `p50 = 158.45 ms`, `p95 = 166.56 ms` (`phase1-baseline.json`) | `p50 = 201.20 ms`, `p95 = 226.51 ms` (`phase2-baseline.json`) | `p50 = 169.67 ms`, `p95 = 198.45 ms` (`benchmarks/results/phase3-baseline.json`) |
| 10,000 requests analysis | `< 5 seconds` | `p50 = 1,328.04 ms`, `p95 = 1,431.39 ms` (`phase1-baseline.json`) | `p50 = 1,981.43 ms`, `p95 = 2,036.57 ms` (`phase2-baseline.json`) | `p50 = 1,650.61 ms`, `p95 = 2,309.88 ms` (`benchmarks/results/phase3-baseline.json`) |
| 50,000 requests analysis | `< 20 seconds` | _Scheduled for load suite_ | _Scheduled for load suite_ | _Scheduled for load suite_ |
| Peak RAM (10k requests) | `< 500 MB` | _RSS profiling scheduled in Phase 8_ | _RSS profiling scheduled in Phase 8_ | _RSS profiling scheduled in Phase 8_ |
| Local CLI startup | `< 300 ms` | `p50 = 23.57 ms`, `max = 34.44 ms` | _Maintained (`< 300 ms`)_ | _Maintained (`< 300 ms`)_ |
| SDK generation (100 endpoints) | `< 2 seconds` | _Scheduled for Phase 5_ | _Scheduled for Phase 5_ | _Scheduled for Phase 5_ |

## Dependency Detection Accuracy (Phase 3, `benchmarks/results/phase3-dependency-eval.json`)

| Dataset | Ground-Truth Edges | Confirmed Precision (Target `>= 98%`) | Direct Recall (Target `>= 85%`) | F1 Score | Un-evidenced Confirmed (Target `0`) |
| ------- | ------------------ | ------------------------------------- | ------------------------------- | -------- | ----------------------------------- |
| Primary (`benchmarks/datasets/dependency-eval-primary.json`) | `29` (`6` scenarios + hard negatives) | `1.0000` (`24/24`, `FP = 0`) | `1.0000` (`29/29`) | `1.0000` | `0` |
| Unseen Holdout (`benchmarks/datasets/dependency-eval-holdout.json`) | `10` (`3` scenarios + hard negatives) | `1.0000` (`7/7`, `FP = 0`) | `1.0000` (`10/10`) | `1.0000` | `0` |

## Running Benchmarks

```bash
cargo run --release -p harflow-project --example phase1_bench -- --out benchmarks/results/phase3-baseline.json
cargo run --release -p harflow-dependencies --example eval_bench -- --out benchmarks/results/phase3-dependency-eval.json
```

Benchmark datasets and baseline outputs live under `benchmarks/datasets/`, `benchmarks/scripts/`, and `benchmarks/results/`.
