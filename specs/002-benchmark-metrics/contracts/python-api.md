# Contract: Python API additions

```python
ds.prom(found, truth, average="micro", off_target="ignore") -> dict
```
- `found`, `truth`: `list[list[tuple[int, int]]]` (start, length) or a `Result` (uses its motifs).
- Returns the `Prom` fields (`precision`, `recall`, `f1`, `tp`, `fp`, `fn`, `sets`, ...).

```python
ds.discover(data, ground_truth="gt.json", ...)   # res.metrics -> dict | None
```

```python
ds.synthetic_suite(dir, n_series=20, n=5000, lens=(40, 60, 80), occ=6, stretch=0.3,
                   noise=0.1, seed=0) -> list[Path]          # GT JSON files
ds.read_truth(path) -> tuple[np.ndarray, list[list[tuple[int, int]]]]
```

```python
b = ds.bench(config, out=None, jobs=None, progress=None, **overrides)   # config: YAML path or dict
# overrides: bare keys set every matching leaf (n_series=2, seeds=[0], scaling=None)
# custom methods: DITSCOVER_METHODS=<module> exposing METHODS = {name: fn} (and DETERMINISTIC)
# cache (UCR zips, TSMD series, yfinance, numba): $DITSCOVER_CACHE or ~/.cache/ditscover
b.runs        # list[dict] (RunRecord)
b.summary     # dict (BenchSummary)
b.plot(path=None, cdn=False)  # HTML report
ds.load_bench(dir) -> Bench
```

```python
res = ds.discover(ticker="^GSPC", start="2015-01-01", end="2025-01-01", interval="1wk",
                  min_len=8, max_len=26)            # needs ditscover[finance]
res.timestamps     # list[str] | None (ISO dates)
ds.discover("prices.csv", column="Close", time_column="Date", ...)   # dates from a CSV
```
`data` becomes optional when `ticker` is given. Unknown ticker / empty download ⇒ `ValueError`;
missing package ⇒ `RuntimeError("... pip install 'ditscover[finance]'")`.

```python
res.plot()    # now the dashboard (cards + motif table + tabs); kind="motifs"... unchanged
```

Errors: invalid bench config ⇒ `ValueError` with key path; missing baseline package ⇒
`RuntimeError("... pip install 'ditscover[bench]'")`.
