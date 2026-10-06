---
description: "Task list: ground-truth metrics, benchmark and dashboard reports"
---

# Tasks: Ground-truth metrics, state-of-the-art benchmark and dashboard reports

**Input**: Design documents from `/specs/002-benchmark-metrics/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: mandatory (Constitution IV). Each test block is written first and MUST fail.

## Format: `[ID] [P?] [Story] Description`

---

## Phase 1: Setup

- [X] T001 Add the `bench` extra (`stumpy`, `dtai-locomotif`) and the `finance` extra (`yfinance`) to `pyproject.toml` and create `python/ditscover/bench/__init__.py`
- [X] T002 Spike (throwaway, not committed): install the extra and `tsmd-evaluation` from git; confirm `apply_locomotif` signature / end exclusivity, `stumpy.motifs` output, whether `tsmd-evaluation` exposes its PROM and benchmark generator as importable functions. Record outcomes in `specs/002-benchmark-metrics/research.md` (R2, R5) and add `tsmd-evaluation` to the extra in `pyproject.toml` if usable

---

## Phase 2: Foundational

- [X] T003 Move `iou` / `match_occurrences` from `crates/core/src/synth.rs` to `crates/core/src/metrics.rs` and update callers and tests

**Checkpoint**: `cargo test` green, no behaviour change

---

## Phase 3: User Story 1 - Score motif sets against ground truth (P1) 🎯 MVP

**Goal**: one shared, verified quality metric (PROM) for any method.

**Independent Test**: hand cases + random corpus equal to the reference PROM (SC-001).

### Tests (first)

- [X] T004 [P] [US1] `crates/core/tests/metrics.rs`: identity ⇒ 1/1/1; overlap exactly 0.5 ⇒ no match; off-target ignore vs penalize; micro vs macro; empty inputs ⇒ zeros; variable-length GT; overlapping occurrences within a found set each match at most one GT occurrence; per-set breakdown and assignment
- [X] T005 [P] [US1] Proptest in `crates/core/src/metrics.rs` (`#[cfg(test)]`): Hungarian equals brute force over permutations for `g, d ≤ 6`; `0 ≤ P, R, F1 ≤ 1`; `TP + FN` = GT occurrences
- [X] T006 [P] [US1] `tests/python/test_prom.py`: `ds.prom` on lists and on a `Result`; oracle against `tsmd-evaluation` on ≥ 1 000 random cases, difference < 1e-9 (skipped without the extra)
- [X] T007 [P] [US1] `crates/core/tests/result.rs` + `tests/python/test_api.py`: `ground_truth` in config ⇒ `metrics` present, saved and reloaded; old `results.json` without `metrics` still loads

### Implementation

- [X] T008 [US1] `GroundTruth` (serde, validation per data-model.md), occurrence matching (overlap > 0.5), Hungarian, `prom(found, gt, avg, off)` in `crates/core/src/metrics.rs`
- [X] T009 [US1] `eval` section (`ground_truth`, `average`, `off_target`) in `crates/core/src/config.rs`; `metrics: Option<Prom>` (`#[serde(default)]`) in `crates/core/src/result.rs`; compute in `discover` in `crates/core/src/lib.rs`
- [X] T010 [US1] Bindings `prom`, `read_truth` in `crates/py/src/lib.rs`; `ds.prom`, `ds.read_truth`, `Result.metrics` in `python/ditscover/__init__.py`

**Checkpoint**: quickstart §1 passes

---

## Phase 4: User Story 2 - Compare with LoCoMotif and Matrix Profile (P1)

**Goal**: one command runs every method on labelled suites in isolation and summarises quality and cost.

**Independent Test**: smoke suite (2 synthetic series, 3 methods, 1 seed) runs end to end.

### Tests (first)

- [X] T011 [P] [US2] `crates/core/tests/bench.rs`: `specs/002-benchmark-metrics/contracts/bench.example.yaml` parses; unknown keys, empty datasets / methods, duplicate labels, unknown TSMD name, non-ascending scaling lengths ⇒ errors with key path; a bare override key (`n_series=2`, `k=3`) sets every matching leaf (all dataset entries and the top level), unknown key ⇒ error; `yfinance` dataset without `min_len` / `max_len` or with `k = 0` ⇒ error; `yfinance` dataset keeps its own `k` (default 5) when the top-level `k` is `oracle`
- [X] T012 [P] [US2] `tests/python/test_methods.py`: each adapter returns `list[list[(start, len)]]` inside the series with ≤ k sets; STOMP pooling keeps non-overlapping sets; series shorter than a method's minimum ⇒ error with reason; missing package ⇒ `RuntimeError` naming the extra
- [X] T013 [P] [US2] `tests/python/test_bench.py` with a fake method registered for tests: isolation (a crashing method ⇒ `error`, a sleeping one ⇒ `timeout`, benchmark continues); resume skips finished keys (and re-runs them when the config hash changes); unavailable method ⇒ error before any run; deterministic methods run once per series; repeated benchmark ⇒ identical quality columns; scaling rows produced with `time_s` and `delta_mb`
- [X] T014 [US2] `tests/python/test_bench.py` summary on a hand-made `runs.csv`: failed / timed-out labelled runs count as P = R = F1 = 0 in means, ranks and `best` and are counted in `failures`; time / memory means use successful runs only; ties ⇒ average rank; stochastic F1 averaged over seeds before ranking; `best` = highest mean F1 over labelled runs (ties → lowest overall rank); a method that fails on the hardest series cannot become `best` by failing
- [X] T015 [P] [US2] `tests/python/test_worker.py`: worker job on stdin ⇒ JSON on stdout with `time_s`, `peak_mb`, `delta_mb` (`delta_mb = max(0, peak after run − peak after warm-up)`, `null` on Windows), `threads`; warm-up excluded from `time_s`
- [X] T016 [P] [US2] `tests/python/test_cli.py`: `ditscover bench` smoke run writes `runs.csv`, `summary.json`, `bench.yaml`; invalid config ⇒ exit 2; missing `bench` extra ⇒ exit 1 naming the extra

### Implementation

- [X] T017 [US2] `BenchConfig`, `DatasetSpec`, `MethodSpec`, `Scaling` + `validate` in `crates/core/src/bench.rs` (per-dataset `k` for `yfinance`, default 5); binding `build_bench_config` in `crates/py/src/lib.rs`
- [X] T018 [P] [US2] `synth::suite` in `crates/core/src/synth.rs` + binding `synthetic_suite` in `crates/py/src/lib.rs`; `ds.synthetic_suite` in `python/ditscover/__init__.py`
- [X] T019 [P] [US2] ditscover (params through `build_config`, `top_k = k`, `seed`), LoCoMotif and STOMP adapters (common params `min_len`, `max_len`, `k`, `seed`; STOMP lengths default to the midpoint, pooled best `k` non-overlapping sets) and availability check in `python/ditscover/bench/methods.py`
- [X] T020 [P] [US2] Synthetic and TSMD-Bench datasets with cache `~/.cache/ditscover/tsmd/` in `python/ditscover/bench/datasets.py` (reference generator or port, per T002)
- [X] T021 [US2] Worker (job on stdin, warm-up on a 4·max_len series, `perf_counter` timing, `ru_maxrss` peak and `delta_mb`, threads env vars) in `python/ditscover/bench/worker.py`
- [X] T022 [US2] Runner (jobs, subprocess + timeout, append `runs.csv`, resume by `(method, dataset, series, seed, cfg_hash)`, progress on stderr) and summary per data-model.md BenchSummary rules (failures scored 0, cost on `delta_mb`, `best`, ranks, scaling medians); `Bench`, `ds.bench`, `ds.load_bench` in `python/ditscover/bench/__init__.py`
- [X] T023 [US2] `ditscover bench` (summary table, exit codes per `specs/002-benchmark-metrics/contracts/cli.md`) in `python/ditscover/cli.py`

**Checkpoint**: quickstart §3 without `--plot`

---

## Phase 5: User Story 3 - Dashboard report for a run (P2)

**Goal**: the run report leads with key numbers and organises charts in tabs.

**Independent Test**: generated run report opened offline shows cards, sortable table and 3 tabs.

- [X] T024 [P] [US3] `tests/python/test_report.py`: run report contains the 6 cards (+ P/R/F1 with GT), one table row per motif with numeric `data-v` sortable by support / cohesion / mean length / stretch, 3 tabs with one panel each, plotly.js embedded exactly once (inline) or CDN `<script>` with `cdn=True`, no external requests otherwise
- [X] T025 [US3] Dashboard shell (cards, sortable table, tabs with resize on show, light / dark theme via `prefers-color-scheme`) and `run_report` in `python/ditscover/report.py`
- [X] T026 [US3] `Result.plot()` default → run dashboard via `report.run_report` in `python/ditscover/__init__.py`; `run --plot` uses it in `python/ditscover/cli.py`

**Checkpoint**: quickstart §2 report checks pass

---

## Phase 6: User Story 4 - Benchmark report (P2)

**Goal**: one page shows which method wins and at what cost.

**Independent Test**: `ditscover plot bench/` writes the benchmark dashboard.

- [X] T027 [P] [US4] `tests/python/test_report.py`: bench report cards (best method by mean F1, runs, failures), Summary table rows = method × dataset with failures column, Quality / Cost tabs with one trace per method, Cost and scaling charts plot `delta_mb`, fixed colour per method; `ditscover plot` detects run vs bench folder
- [X] T028 [US4] Figures `quality`, `cost` (time and `delta_mb`, log axis), `scaling` (log-log) in `python/ditscover/plot.py`
- [X] T029 [US4] `report.bench_report` in `python/ditscover/report.py`; `Bench.plot` in `python/ditscover/bench/__init__.py`; `bench --plot` and folder detection in `ditscover plot` in `python/ditscover/cli.py`

**Checkpoint**: quickstart §3 with `--plot`

---

## Phase 7: User Story 5 - Market data from Yahoo Finance (P3)

**Goal**: motifs on a ticker's price series, with dates in reports, cached locally.

**Independent Test**: with `yf.download` monkeypatched, a ticker run returns motifs and a dated
report; the second run reads the cache.

### Tests (first)

- [X] T030 [P] [US5] `crates/core/tests/config.rs`: market-data fields and defaults; `path` + `ticker` ⇒ error; `period` + `start` ⇒ error; invalid interval / date ⇒ error with key path; leaf overrides `ticker`, `interval`; `ticker` without values passed to `discover` ⇒ error naming the Python package
- [X] T031 [P] [US5] `crates/core/tests/io.rs` + `crates/core/tests/result.rs`: `time_column` read as strings; `timestamps` saved as first column of `series.csv` and reloaded; old results without timestamps load
- [X] T032 [P] [US5] `tests/python/test_sources.py` (no network): fetch → values + ISO timestamps; cache hit on second call; open-ended `end` resolved and written to the effective config; `refresh` re-downloads; empty frame / unknown ticker / too-old intraday range ⇒ `ValueError`; missing yfinance ⇒ `RuntimeError` naming `finance`; optional `@pytest.mark.network` real download skipped by default
- [X] T033 [P] [US5] `tests/python/test_report.py`: with timestamps, time axes, tooltips and motif table use dates; `tests/python/test_cli.py`: `--ticker` shortcuts, `--data` + `--ticker` ⇒ exit 2
- [X] T034 [P] [US5] `tests/python/test_bench.py`: `yfinance` dataset (monkeypatched download) ⇒ runs with `k = 5`, `n_sets`, cost columns and empty quality; absent from `rank` and `best`; its failures still counted

### Implementation

- [X] T035 [US5] Market-data fields, `time_column` and validation in `crates/core/src/config.rs`; `timestamps` in `crates/core/src/result.rs` and `crates/core/src/io.rs`; `discover(values, cfg, timestamps)` in `crates/core/src/lib.rs` and `crates/py/src/lib.rs`
- [X] T036 [US5] `python/ditscover/sources.py` (download, cache `~/.cache/ditscover/yfinance/`, resolve end) and its use in `ds.discover` (`python/ditscover/__init__.py`) and `ditscover run` shortcuts (`python/ditscover/cli.py`)
- [X] T037 [US5] Dates in figures (`python/ditscover/plot.py`) and motif table / cards / tooltips (`python/ditscover/report.py`)
- [X] T038 [US5] `yfinance` dataset in `python/ditscover/bench/datasets.py` (dataset `k`, `min_len`, `max_len` passed to jobs); quality null and excluded from ranks and `best` in `python/ditscover/bench/__init__.py`; Quality tab skips these datasets in `python/ditscover/report.py`

**Checkpoint**: quickstart §4 passes

---

## Phase 8: Polish

- [X] T039 Run `specs/002-benchmark-metrics/contracts/bench.example.yaml` in full (including the market-data dataset), check SC-002 (< 15 min for the synthetic suite on 8 cores), review `report.html` at 1280×800 in light and dark (SC-005) and keep `runs.csv` / `summary.json` under `results/bench-001/`
- [X] T040 [P] CI: smoke benchmark job with the `bench` extra (2 series, 1 seed, no scaling) in `.github/workflows/ci.yml`
- [X] T041 Quickstart `specs/002-benchmark-metrics/quickstart.md` end to end (including the forced-timeout check); `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `ruff check`, `ruff format` clean; `cargo bench -p ditscover-core --bench ga -- --baseline pre002` shows no regression > 10 % (Constitution VI)

---

## Dependencies & Execution Order

- T001–T002 → T003 → US1 (T004–T010).
- US2 needs US1 (PROM). US3 needs only US1 (cards with P/R/F1). US4 needs US2 + US3 (shell).
- US5: T030–T033, T035–T036 need only Phase 2 and can start any time; T037 needs US3 (shell);
  T034 and T038 need US2.
- Within each story: tests → core → bindings → Python.

## Parallel Example: US2

```text
T011 bench.rs | T012 test_methods.py | T013 + T014 test_bench.py | T015 test_worker.py | T016 test_cli.py
then: T017 bench.rs → T018 synth suite ‖ T019 methods.py ‖ T020 datasets.py ‖ T021 worker.py → T022 → T023
```

## Parallel Example: US5

```text
T030 config.rs | T031 io.rs + result.rs | T032 test_sources.py | T033 test_report.py + test_cli.py
then: T035 → T036 → T037 (after US3) and T038 (after US2)
```

## Implementation Strategy

1. MVP = US1: one shared, verified metric.
2. US2: the comparison itself (synthetic first, TSMD-Bench next).
3. US3 can start in parallel with US2; US4 closes the loop.
4. US5 (market data) is independent and can be delivered first if a demo on real data is wanted.
