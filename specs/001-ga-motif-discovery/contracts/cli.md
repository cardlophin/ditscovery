# Contract: CLI (`ditscover`)

```
ditscover run [CONFIG.yaml] [--data PATH] [--column NAME|IDX] [--seed N] [--out DIR]
              [--set KEY=VALUE ...] [--plot] [--cdn] [-q|-v]
ditscover plot DIR [--kind all|motifs|overlay|evolution|front] [--out FILE.html] [--cdn]
```

- `run`: `--data`, `--column`, `--seed`, `--out` are shortcuts for `--set path=`, `--set column=`,
  `--set seed=`, `--set dir=`. `--set` takes leaf names and YAML values
  (`--set preprocess='[{nan: drop}]'`). Precedence: defaults < YAML < `--set`/shortcuts.
- Output in `dir` (CLI default `./ditscover-out/`): `results.json`, `history.jsonl` (streamed
  during the run), `config.yaml`, `series.csv`, and `report.html` if `--plot`.
- `plot DIR` needs only the files in `DIR` (the series comes from `series.csv`); default output
  `DIR/report.html`.
- HTML is self-contained and works offline; `--cdn` loads Plotly from the CDN instead.
- stdout: summary table of the top-K (support, cohesion, mean length, stretch).
- stderr: progress (`tracing`), unless `-q`.
- Exit codes: `0` ok · `1` data/IO error · `2` invalid config · `130` cancelled (partial result
  written).
