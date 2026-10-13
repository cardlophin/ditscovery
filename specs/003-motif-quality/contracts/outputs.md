# Contract: output changes

## Run folder

- `results.json`: motifs gain `extent`, `margin`, `coverage`, `quality`. Labels are stored in
  `series.csv`, not here.
- `series.csv`: columns `[timestamp,]value[,label]`; `label` is the motif number (1-based) or empty.

## Benchmark folder

- `sets.jsonl`: one line per finished run `{"key": [...], "sets": [[[s, l], ...], ...]}`.
- `summary.json`: adds `tuned`, `tests`, `ablation`, `decisions` (see data-model.md); rows carry
  `split`.
- `report.html`: tabs Summary / Quality / Cost / **Errors** / **Ablation** (Ablation only when
  `full` and `full-*` variants exist).

## Run report

- Labels band under the series chart (one colour per motif, grey = none).
- Motif table gains `margin`, `extent`, `coverage`, `quality` columns; sorted by `quality` by default.
