# Quickstart / Validation

## Requirements

Stable Rust (edition 2024), `uv`, Python 3.12.

```bash
uv venv && uv pip install -e '.[dev]'      # maturin, pytest, ruff, pandas
uv run maturin develop --release
```

## 1. Core (US1, SC-001, SC-003)

```bash
cargo test -p ditscover-core                      # invariants, determinism, validation, persistence
cargo test -p ditscover-core --release planted    # SC-001: recall ≥ 0.9, precision ≥ 0.8 (IoU ≥ 0.5)
```

## 2. Python API (US1, US2, US4, US5)

```python
import ditscover as ds
res = ds.discover("data/kaggle-electric-production.csv", column="IPG2211A2N",
                  min_len=10, max_len=16, preprocess=[{"detrend": "linear"}], seed=1)
res.plot()
```

Expected: ≥ 1 motif with ~12-month occurrences (yearly cycle), `res.stop` set, figure with the
coloured series. Same seed again ⇒ identical `res.motifs`.

Cancellation: `ds.discover(..., max_generations=10**9)` + Ctrl+C ⇒ result with
`stop == "cancelled"` within ≤ 1 s.

## 3. CLI and persistence (US3, US4)

```bash
ditscover run specs/001-ga-motif-discovery/contracts/config.example.yaml --seed 7 --out out/ --plot
ditscover run specs/001-ga-motif-discovery/contracts/config.example.yaml --seed 7 --out out2/
diff <(jq -S .motifs out/results.json) <(jq -S .motifs out2/results.json)   # empty
ls out/            # results.json history.jsonl config.yaml series.csv report.html
ditscover run --set max_len=5 --set min_len=8 ...                           # exit 2
```

Plot without the source data (series restored from `series.csv`), then open the HTML with the
network disabled — all four views must render (offline by default):

```bash
mv data data.bak && ditscover plot out/ --out offline.html; mv data.bak data
```

## 4. Benchmarks (SC-002, SC-004, SC-006, SC-008)

Reference machine: Apple M4 (4P+6E), 16 GB, macOS 26.

```bash
cargo bench -p ditscover-core --bench ga -- --save-baseline ref    # once, on main
cargo bench -p ditscover-core --bench ga -- --baseline ref         # on the branch
uv run python scripts/bench_check.py                    # exit 1 if any bench > +10 %
```

Expected: `ga/run_10k` < 30 s, `ga/threads_8` ≥ 5× faster than `ga/threads_1`, `ga/log_on`
< 5 % slower than `ga/log_off`.

## 5. Full suite

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
uv run ruff check && uv run ruff format --check && uv run pytest tests/python
```

References: [python-api](contracts/python-api.md), [cli](contracts/cli.md),
[outputs](contracts/outputs.md), [config](contracts/config.example.yaml),
[data model](data-model.md).
