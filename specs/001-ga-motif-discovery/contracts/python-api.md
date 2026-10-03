# Contract: Python API (`import ditscover as ds`)

```python
ds.discover(data, config=None, **overrides) -> ds.Result
```

- `data`: path (`str | Path`) to CSV/TSV, 1-D `np.ndarray`, `list[float]` or `pd.Series`
  (converted with `np.asarray`; pandas is not a runtime dependency).
- `config`: path to YAML, `dict` or `None` (defaults).
- `overrides`: Config leaf names (`min_len=12, population=300, seed=7, column="x", kind="dtw",
  band=0.1, dir="out/", preprocess=[{"nan": "interpolate"}, {"paa": 2}]`).
- Releases the GIL; `KeyboardInterrupt` ⇒ returns the partial result (`stop == "cancelled"`).
- Config/data errors ⇒ `ValueError` with the key path; file errors ⇒ `OSError`.
- If `dir` is set, the run streams `history.jsonl` there and writes the full result at the end
  (same as `res.save(dir)`).

```python
res.motifs          # list[Motif], top-K distinct, ordered by (Pareto rank, distance to ideal)
res.front           # list[Motif], first front
res.history         # list[dict] (GenerationRecord)
res.stop            # "max_generations" | "time_budget" | "stagnation" | "cancelled"
res.config          # effective dict (with seed)
res.series          # np.ndarray, original series (NaN kept); restored by ds.load
res.plot(kind="all" | "motifs" | "overlay" | "evolution" | "front", path=None, cdn=False)
                    # path=None ⇒ Figure; path ⇒ writes self-contained HTML (offline unless cdn=True)
res.save(dir)       # results.json, history.jsonl, config.yaml, series.csv
ds.load(dir) -> Result

m.occurrences       # list[tuple[int, int]] (start, length) in original indices
m.prototype         # np.ndarray
m.aligned           # np.ndarray (k × L*) or None (front motifs)
m.support, m.cohesion, m.mean_len, m.stretch, m.objectives
```

Minimal use (SC-005):

```python
res = ds.discover("data.csv", column="IPG2211A2N", min_len=10, max_len=16, seed=1)
res.plot()
```
