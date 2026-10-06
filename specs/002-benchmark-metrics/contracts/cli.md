# Contract: CLI additions

```
ditscover bench CONFIG.yaml [--out DIR] [--set KEY=VALUE ...] [--jobs N] [--plot] [--cdn] [-q|-v]
ditscover plot DIR [--out FILE.html] [--cdn]      # DIR may be a run or a benchmark folder
ditscover run ... [--set ground_truth=gt.json]    # adds PROM to results and report
ditscover run [CONFIG] --ticker SYMBOL [--start YYYY-MM-DD] [--end YYYY-MM-DD] [--interval 1d]
              [--set field=Close --set refresh=true ...]   # market data (ditscover[finance])
```

- `bench` output in `--out` (default `./ditscover-bench/`): `runs.csv`, `summary.json`,
  `bench.yaml` (effective config + method versions), `datasets/` (GT files of synthetic suites),
  `report.html` with `--plot`.
- stdout: summary table (label, dataset, F1 mean ± std, time mean, rank).
- stderr: one progress line per finished run (`[12/180] stomp synthetic#3 seed=- ok F1=0.82 1.4s`).
- Exit codes: `0` ok (even with failed runs) · `1` data / IO / missing extra · `2` invalid config ·
  `130` interrupted (finished runs kept).
- `--ticker/--start/--end/--interval` are shortcuts of `--set`; `--data` and `--ticker` are mutually
  exclusive (exit 2).
- `plot` detects the folder type (`results.json` ⇒ run report, `runs.csv` ⇒ benchmark report).
