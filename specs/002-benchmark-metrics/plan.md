# Implementation Plan: Ground-truth metrics, state-of-the-art benchmark and dashboard reports

**Branch**: `002-benchmark-metrics` | **Date**: 2026-10-05 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/002-benchmark-metrics/spec.md`

## Summary

Add PROM (precision-recall under optimal matching) to the Rust core as the single quality metric
for any method; a Python benchmark harness that runs ditscover, LoCoMotif and Matrix Profile
(STOMP) on labelled suites (synthetic variable-length + TSMD-Bench univariate) and unlabelled
market data in isolated subprocesses, measuring quality and cost, with resumable outputs; and a shared dashboard shell
(summary cards, sortable table, tabs) for run and benchmark reports. Market data from Yahoo
Finance (yfinance) becomes an optional data source with local caching and dates carried into
results and reports. Failed runs score 0 in quality summaries and ranks; cost comparisons use memory
above the post-warm-up baseline; market-data datasets use a fixed `k` (default 5). Details in
[research.md](research.md).

## Technical Context

**Language/Version**: Rust 1.85+ (edition 2024); Python ≥ 3.12

**Primary Dependencies**: no new Rust crates; Python extra `bench`: `stumpy`, `dtai-locomotif`,
`tsmd-evaluation` (git, if the spike keeps the reference generator / oracle); extra `finance`:
`yfinance`

**Storage**: files (`runs.csv`, `summary.json`, `bench.yaml`, GT JSON, `report.html`); cache in
`~/.cache/ditscover/{tsmd,yfinance}`

**Testing**: `cargo test` + `proptest` (Hungarian vs brute force, PROM invariants); `pytest`
(oracle PROM, adapters, runner resume / timeout, reports)

**Target Platform**: Linux, macOS (memory measurement); Windows without memory figures

**Project Type**: library + CLI (feature on top of 001)

**Performance Goals**: synthetic suite 20 series × 3 methods × 3 seeds < 15 min on 8 cores;
measurement overhead outside the timed call only

**Constraints**: identical quality metrics on repetition; finished runs never re-executed;
reports self-contained and offline by default

**Scale/Scope**: ≤ ~20 GT sets per series; series up to 2·10⁴ points in benchmarks

## Constitution Check

| Principle | Compliance |
|---|---|
| I. Pure Rust core | PROM, GT model, BenchConfig and market-data config validation in `crates/core`; Python only orchestrates processes, fetches market data, adapts third-party methods and renders ✅ (see Complexity Tracking) |
| II. Friendly Python API | `ds.prom`, `ds.bench`, `ds.synthetic_suite`, `res.metrics`, `res.plot()` ✅ |
| III. Minimal code | own Hungarian (no crate), one HTML shell for both reports, no Jinja / Dash ✅ |
| IV. Test-first | Hungarian vs brute force, PROM oracle (SC-001), runner tests with fake methods ✅ |
| V. Reproducibility | seeds per run, deterministic baselines, versions and config hash stored ✅ |
| VI. Performance | timings exclude warm-up; threads controlled and recorded ✅ |
| VII. Decoupled observability | benchmark progress on stderr; GA logging unchanged ✅ |
| VIII. Single configuration | `BenchConfig` parsed and validated in Rust; ditscover params via `build_config` ✅ |

Post-design re-check: no violations.

## Project Structure

### Documentation (this feature)

```text
specs/002-benchmark-metrics/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/ (python-api.md, cli.md, ground-truth.md, outputs.md, bench.example.yaml)
└── tasks.md
```

### Source Code (repository root)

```text
crates/core/src/
├── metrics.rs          # overlap rate, occurrence matching, hungarian, prom, GroundTruth
├── bench.rs            # BenchConfig, DatasetSpec, MethodSpec, validate
├── config.rs           # + eval section, market-data and time_column fields
├── result.rs           # + metrics: Option<Prom>, timestamps
├── synth.rs            # iou/match_occurrences moved to metrics.rs; + suite()
└── lib.rs              # discover computes metrics when eval.ground_truth is set
crates/core/tests/ (metrics.rs, bench.rs)
crates/py/src/lib.rs    # + prom, read_truth, synthetic_suite, build_bench_config
python/ditscover/
├── __init__.py         # + prom, synthetic_suite, read_truth, bench, load_bench; Result.metrics/.timestamps
├── sources.py          # yfinance fetch + cache → (values, timestamps)
├── report.py           # dashboard shell: cards, sortable table, tabs, theme, plotly once
├── plot.py             # existing figures + bench figures (quality, cost, scaling)
├── cli.py              # + bench; plot detects folder type
└── bench/
    ├── __init__.py     # Bench, run(), resume, summary, ranks
    ├── methods.py      # ditscover / locomotif / stomp adapters + availability check
    ├── datasets.py     # synthetic suites, TSMD-Bench (cache)
    └── worker.py       # subprocess entry: warm-up, timing, memory
tests/python/ (test_prom.py, test_methods.py, test_bench.py, test_report.py, test_sources.py)
```

**Structure Decision**: extends the 001 layout; the benchmark is a Python subpackage because it
orchestrates external Python libraries and processes.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| STOMP multi-length pooling (≈15 lines) in Python | adapts a third-party output to the common interface | moving it to Rust would require passing stumpy results through the bindings for no gain |
| Market-data fetching in Python (`sources.py`) | the provider is a Python library; config and validation stay in Rust | a Rust HTTP client would re-implement an unofficial, changing API |
