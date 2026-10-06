# Data Model

## Core (Rust)

### GroundTruth (`metrics.rs`, serde)

| Field | Type | Rule |
|---|---|---|
| `name` | `String` | optional label |
| `series` | `Vec<f64>` | length ≥ 1 (NaN allowed, preprocessing applies) |
| `sets` | `Vec<Vec<(u32, u32)>>` | each occurrence inside the series; occurrences of all sets pairwise non-overlapping |

### Prom (`metrics.rs`, serde)

| Field | Type |
|---|---|
| `tp`, `fp`, `fn_` (serialised `fn`) | `u32` |
| `precision`, `recall`, `f1` | `f64` (0 when undefined) |
| `sets` | `Vec<SetScore { gt: usize, found: Option<usize>, tp, fp, fn_, precision, recall, f1 }>` |
| `average` | `Average = Micro \| Macro` |
| `off_target` | `OffTarget = Ignore \| Penalize` |

`pub fn prom(found: &[Vec<(u32,u32)>], gt: &[Vec<(u32,u32)>], avg, off) -> Prom`.

### Config additions (`config.rs`)

`eval: { ground_truth: Option<PathBuf> (null), average: micro|macro (micro), off_target:
ignore|penalize (ignore) }` — leaf names unique (overrides `ground_truth=...`).

### Data additions (`config.rs`)

`data: { ..., time_column: Option<Column> (null), ticker: Option<String> (null), start:
Option<String> (null), end: Option<String> (null), period: Option<String> (null), interval: String
("1d"), field: String ("Close"), refresh: bool (false) }`. Rules: `path` xor `ticker` (or neither
when an array is passed); `period` xor `start`; `interval` ∈ {1m, 2m, 5m, 15m, 30m, 60m, 90m, 1h,
1d, 5d, 1wk, 1mo, 3mo}; dates `YYYY-MM-DD`.

### RunResult additions (`result.rs`)

`metrics: Option<Prom>` and `timestamps: Option<Vec<String>>` (both `#[serde(default)]`, older
results still load). `series.csv` gains a leading `timestamp` column when present.

### BenchConfig (`bench.rs`, serde, `deny_unknown_fields`)

| Field | Type | Default / rule |
|---|---|---|
| `datasets` | `Vec<DatasetSpec>` | ≥ 1, unique names |
| `methods` | `Vec<MethodSpec>` | ≥ 1, unique labels |
| `seeds` | `Vec<u64>` | `[0, 1, 2]` |
| `k` | `oracle \| u32` | `oracle` |
| `timeout_s` | `f64` | 600 |
| `threads` | `Option<usize>` | all cores |
| `jobs` | `usize` | 1 |
| `eval` | `average`, `off_target` | as in Config |
| `scaling` | `Option<Scaling { lengths: Vec<usize>, repeats: u32 (3), methods: Option<Vec<String>> }>` | lengths ascending |

`DatasetSpec` (externally tagged):
- `synthetic: { name, n_series (20), n (5000), lens ([40, 60, 80]), occ (6), stretch (0.3), noise (0.1), seed (0) }`
- `tsmd: { name: ECG5000|Fungi|Plane|Mallat|Symbols, split: test|validation (test), n_series (20) }`
- `files: { name, paths: Vec<PathBuf> }` (GT JSON files)
- `yfinance: { name, tickers: Vec<String>, start, end, period, interval ("1d"), field ("Close"), min_len, max_len, k (5) }` (no GT; cost only). Its own fixed `k` always applies: the top-level `k: oracle` is ignored for it; `min_len` / `max_len` required; `k ≥ 1`.

Overrides (`--set KEY=VALUE`): a bare key sets every matching leaf of the config tree (top level and
every dataset / method entry); a key with no match is an error.

`MethodSpec`: `{ method: String, label: Option<String> (= method, filled in), params: Map<String, Value> ({}) }`.
`method` is checked in Python against the adapter registry (`ditscover`, `locomotif`, `stomp`, plus
the `METHODS` / `DETERMINISTIC` of the module named by `DITSCOVER_METHODS`, used by the tests and for
custom methods) before any run; the core only checks the structure.
Common params injected per series: `min_len`, `max_len` (from the dataset: synthetic `lens` ± stretch,
TSMD the class-length range; overridable in `params`), `k`, `seed`.

## Python (benchmark runner)

### Job (stdin of the worker)

`{ method, label, params, series: [f64], k, seed, min_len, max_len, threads }`

### RunRecord (one row of `runs.csv`)

`method, label, dataset, series, seed, cfg_hash, status (ok|error|timeout), reason, precision,
recall, f1, tp, fp, fn, n_sets, time_s, peak_mb, delta_mb, threads, n` — quality fields empty for
datasets without GT and for failed runs (the summary scores failed runs on labelled datasets as 0).

### BenchSummary (`summary.json`)

`{ by_method_dataset: [{label, dataset, n_runs, failures, mean/std of precision, recall, f1, time_s,
peak_mb, delta_mb}], rank: {label: {dataset: avg_rank, "overall": avg_rank}}, best: label, scaling:
[{label, n, time_s, delta_mb}], versions: {method: version}, config: {...} }`

Rules: failed runs on labelled datasets → P = R = F1 = 0 in means, ranks and `best`; time / memory
means over successful runs; datasets without GT have null quality and are absent from `rank`;
`best` = highest mean F1 over labelled runs (ties → lowest overall rank). Cost comparisons use `delta_mb`.

## State of a benchmark run

`pending → running → ok | error | timeout`; persisted on completion (append to `runs.csv`), so an
interrupted benchmark resumes from the last finished run.
