# Quickstart / Validation

## Setup

```bash
uv pip install -e '.[dev,bench]'
uv run maturin develop --release
```

## 1. PROM (US1, SC-001)

```bash
cargo test -p ditscover-core metrics          # hand cases, Hungarian vs brute force
uv run pytest tests/python/test_prom.py       # oracle vs tsmd-evaluation, ≥ 1 000 cases
```

```python
import ditscover as ds
ds.prom([[(0, 10), (50, 10)]], [[(0, 10), (50, 10)]])["f1"]    # 1.0
ds.prom([[(5, 10)]], [[(0, 10)]])["f1"]                        # 0.0 (overlap = 1/3)
```

## 2. Run with ground truth (US1 scenario 5, US3)

```python
files = ds.synthetic_suite("gt", n_series=1, seed=3)
x, sets = ds.read_truth(files[0])
res = ds.discover(x, ground_truth=files[0], min_len=28, max_len=104, top_k=len(sets), seed=1)
res.metrics["f1"]
res.plot(path="report.html")     # cards incl. P/R/F1, motif table, 3 tabs
```

Open `report.html` offline: cards and table visible at 1280×800; switching tabs shows full-width
charts; dark system theme ⇒ dark charts.

## 3. Benchmark (US2, US4)

```bash
ditscover bench specs/002-benchmark-metrics/contracts/bench.example.yaml --out bench/ --plot
# Ctrl+C halfway, then:
ditscover bench specs/002-benchmark-metrics/contracts/bench.example.yaml --out bench/   # resumes
ditscover plot bench/
```

Expected: `runs.csv` without duplicated keys, `summary.json` with ranks, report with Summary /
Quality / Cost tabs. Smoke suite for CI: `--set n_series=2 --set seeds=[0] --set scaling=null`.
With `--set timeout_s=0.001`, every labelled run is `timeout`, `failures` = `n_runs` and mean F1 = 0
for every method (failures score 0). The `indices-weekly` rows have `k = 5`, empty quality and no
rank. Cost columns and scaling curves show `delta_mb`.

## 4. Market data (US5)

```bash
uv pip install -e '.[finance]'
ditscover run --ticker ^GSPC --start 2015-01-01 --end 2025-01-01 --interval 1wk \
  --set min_len=8 --set max_len=26 --out results/sp500-weekly --plot
```

Expected: report with dates on the time axis and in the motif table; rerunning without network
reuses `~/.cache/ditscover/yfinance/` and gives the same motifs. Tests: `uv run pytest
tests/python/test_sources.py` (no network).

## 5. Gates

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
uv run ruff check && uv run pytest
```
