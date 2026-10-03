# Contract: output files

A results directory always contains the four files below; `report.html` is optional.

## history.jsonl — one line per event

```json
{"event":"start","version":"0.1.0","config":{...}}
{"event":"generation","gen":0,"elapsed_s":0.02,"evals":400,"hv":0.41,"front_size":17,"best":[14,0.21,15.2],"mean":[6.1,0.48,12.7],"front":[[...]]}
{"event":"finish","reason":"stagnation","dropped":0}
```

`front` only every `snapshot_every` generations (list of `[support, cohesion, length]`).

## results.json

Serialisation of `RunResult` (data-model.md) without `history` (in `history.jsonl`) and without
`series` (in `series.csv`). Top-K motifs include `aligned`; front motifs have `aligned: null`.

## config.yaml

Full effective config (defaults + YAML + overrides), including `seed`.

## series.csv

Original series before preprocessing: header `value`, one row per point, missing values as `NaN`.
Read back with the core `CsvLoader`.

## report.html

Self-contained Plotly report; embeds plotly.js (offline) unless `--cdn` / `cdn=True`.
