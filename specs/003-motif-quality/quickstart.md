# Quickstart / Validation

## Setup

```bash
uv pip install -e '.[dev,bench]' && uv run maturin develop --release
```

## 1. Corrected baseline (US1)

```bash
ditscover bench specs/002-benchmark-metrics/contracts/bench.example.yaml --out results/bench-002 --plot
```

Expected: STOMP `n_sets` up to `k`; LoCoMotif F1 > 0 on `synth-varlen`; `sets.jsonl` present;
Errors tab shows hits / false alarms / misses; `summary.json` has `tests`. This is the baseline
recorded in research.md.

## 2. Fitness v2 unit behaviour (US2)

```bash
cargo test -p ditscover-core --test fitness --test index
```

Hand-built series: true set outranks set + junk (quality), set − one has a lower margin, a tiling has
no quality; k = 2 not halved.

## 3. Search (US3)

```bash
cargo test -p ditscover-core --test refine --test ops
```

Shifted / resized occurrences (≤ 15 %) recover planted boundaries ± 2 points on noise-free data.

## 4. Labelling (US4)

```python
import ditscover as ds
res = ds.discover("data/kaggle-electric-production.csv", config="specs/003-motif-quality/contracts/config.v2.example.yaml")
occ = sorted(o for m in res.motifs for o in m.occurrences)
assert all(s + n <= t for (s, n), (t, _) in zip(occ, occ[1:]))   # conflict-free
res.labels[:24]; res.plot(path="report.html")
```

Expected: no overlapping occurrences across motifs; top motifs with different shapes; labels band.

## 5. Ablation (US5)

```bash
ditscover bench specs/003-motif-quality/contracts/ablation.example.yaml --out results/ablation-003 --plot
```

Expected: Ablation tab with ΔF1 per component × dataset × split; `summary.json` `decisions` lists
keep / delete per component (validation only, Holm within each dataset). The full 18-variant study
(19 440 runs, ~7.5 h with 5 jobs) is in `results/ablation-003` and research.md (R8); the contract now
holds only the surviving components (7 560 runs).

```bash
ditscover bench specs/003-motif-quality/contracts/final.example.yaml --out results/bench-003 --plot
```

Expected: pruned defaults vs tuned baselines, test evaluated once (SC-001..SC-003).

## 6. Run time (SC-005)

```bash
cargo bench -p ditscover-core -- v2_defaults_20k
uv run pytest -m slow tests/python/test_perf.py
```

Expected: library defaults, one seed, all cores, 20 000 points ⇒ median of 5 seeds < 10 s.

## 7. Gates

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
uv run ruff check && uv run pytest
```
