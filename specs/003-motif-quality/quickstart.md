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
cargo test -p ditscover-core fitness index
```

Hand-built series: true set ≻ set + junk, true set ≻ set − one, true set ≻ tiling; k = 2 not halved.

## 3. Search (US3)

```bash
cargo test -p ditscover-core refine ops archive
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

Expected (~10 h sequential, overnight): Ablation tab with ΔF1 per component × dataset × split;
`summary.json` `decisions` lists keep / delete per component (validation only, Holm within each
dataset) and the `v1` decision; decisions copied to research.md (R8), losing components deleted.

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
