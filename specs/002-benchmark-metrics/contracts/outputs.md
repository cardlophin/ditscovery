# Contract: benchmark output folder

- `runs.csv`: one RunRecord per line (columns in data-model.md); appended as runs finish.
- `summary.json`: BenchSummary; rewritten at the end and on resume.
- `bench.yaml`: effective BenchConfig + `versions` of every method.
- `datasets/<dataset>/<i>.json`: GT files of generated synthetic suites (TSMD lives in the cache).
- `report.html`: benchmark dashboard (with `--plot` or `ditscover plot DIR`).

Run folders (feature 001) gain `metrics` in `results.json` when a GT file was given.
