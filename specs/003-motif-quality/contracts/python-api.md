# Contract: Python API changes

```python
res = ds.discover(x, min_len=40, max_len=100, top_k="auto")      # fitness v2 by default
res.labels            # np.ndarray[int] | None, one label per original index (-1 = none)
m = res.motifs[0]
m.extent, m.margin, m.coverage, m.quality                        # new fields
ds.discover(x, model="v1", rank_by="ideal", ...)                 # previous behaviour (until removed)
ds.discover(x, kind="zdtw", band=0.1, ...)                       # normalised DTW, medoid prototype
```

```python
b = ds.bench("ablation.yaml", out="results/ablation-003")
b.summary["tuned"]      # chosen params per method and dataset
b.summary["tests"]      # paired Wilcoxon results (Holm-adjusted)
b.summary["ablation"]   # ΔF1 per removed component, dataset and split
b.summary["decisions"]  # keep / delete per component (validation only), plus v1
b.sets(key)             # stored sets of one run
```

Errors: invalid new leaves ⇒ `ValueError` with key path; grid too large ⇒ `ValueError` with the
estimated number of runs and `max_runs`.
