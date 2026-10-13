# Data Model

## Config additions (`crates/core/src/config.rs`)

All leaf names unique (flat overrides keep working). Defaults in brackets.

### `fitness` (new section)

| Leaf | Type | Default | Rule |
|---|---|---|---|
| `model` | `v1 \| v2` | `v2` | v1 = feature 001 objectives (support, cohesion, length) |
| `extent_q` | f64 | 0.9 | (0, 1] |
| `null_pairs` | u32 | 256 | ≥ 16 |
| `min_std_rel` | f64 | 0.05 | [0, 1) |
| `cid` | bool | false | |
| `loo` | bool | true | ablation switch |
| `margin` | bool | true | ablation switch (false ⇒ f3 = cohesion as in v1) |
| `coverage` | bool | true | ablation switch (false ⇒ support + length as in v1) |
| `scale` | bool | true | ablation switch (false ⇒ `c_ref` normalisation) |

### `ga` additions

`refine` (0.3), `refine_radius` (0.1), `phase` (0.2), `rescale` (0.15), `guarded_add` (true),
`mating_tau` (0.5, `null` = off), `split` (0.05), `seed_nn` (0.4), `archive` (null = 2·population,
0 = off), `immigrants` (0.1).

### `stop` additions

`min_generations` (100); defaults change: `stagnation_window` 50, `stagnation_delta` 1e-3
(relative to archive HV).

### `output` additions / changes

`top_k`: `usize | "auto"`; `rank_by`: `quality | ideal` (quality); `dedupe_overlap` (0.5);
`dedupe_shape` (0.3); `labels` (true). `dedupe_jaccard` kept for `rank_by: ideal`.

### `distance.kind`

Adds `zdtw`.

## Core types

### `CandidateIndex` (`crates/core/src/index.rs`)

`{ lengths: Vec<u32>, stride: Vec<u32>, sketches: Vec<Vec<[f32; 16]>>, starts: Vec<Vec<u32>> }` —
built once per run; `nearest_outsider(proto, l, taken, ctx) -> Option<(f64, Segment)>`,
`nn_pairs(m) -> Vec<(f64, Segment)>`.

### `NullScale` (`crates/core/src/fitness.rs`)

`{ lengths: [u32; 8], delta: [f64; 8] }`, `at(l) -> f64` (linear interpolation).

### `Raw` (extended, `motif.rs`)

| Field | Type | Note |
|---|---|---|
| `support`, `cohesion`, `mean_len`, `stretch` | as today | |
| `extent` | f64 | quantile of LOO member distances |
| `outsider` | f64 | nearest-outsider distance |
| `margin` | f64 | normalised |
| `coverage` | f64 | |
| `quality` | f64 | `coverage · max(margin, 0)` |
| `member` | `Vec<f64>` | LOO distance per occurrence (labelling weights, refinement) |

### `Archive` (`crates/core/src/archive.rs`)

`Vec<Individual>` bounded; `insert(ind)`, `hv()`, `members()`.

### `Motif` (output) additions

`extent`, `margin`, `coverage`, `quality` (all `#[serde(default)]`).

### `RunResult` additions

`labels: Option<Vec<i32>>` (`#[serde(default)]`; saved as a `label` column in `series.csv`).

## Benchmark additions (`crates/core/src/bench.rs`)

| Field | Type | Default |
|---|---|---|
| `tune` | `Option<{ n_series: u32 (10), metric: f1 }>` | null |
| `max_runs` | u32 | 20 000 |
| `stats` | bool | true |
| `save_sets` | bool | true |
| `MethodSpec.grid` | `Map<String, Vec<Value>>` | {} |
| `SyntheticSpec.split` | `validation \| test` | test |

Synthetic validation split: same generator with `seed + 1_000_000`. Datasets are identified by
`(name, split)`, so one config can list the same dataset on both splits; run keys, `sets.jsonl` and
summary rows carry the split. `max_runs` counts every run (grid tuning + evaluation) and the guard
reports the estimate.

Validation: TSMD `n_series` ≤ 50 on validation (unchanged); the ablation uses 30.

## Outputs

- `sets.jsonl`: `{ key, sets: [[[start, len], ...], ...] }` per run.
- `summary.json`: + `tuned: {label: {dataset: params}}`, `tests: [{dataset, split, a, b, n,
  statistic, p, p_holm}]` (Holm across method pairs within `(dataset, split)`),
  `ablation: [{component, dataset, split, delta_f1, ci_low, ci_high, p, p_holm}]` (Holm across
  components within `(dataset, split)`) and `decisions: [{component, keep, reason}]` (validation
  rows only; `component: v1` compares `full` vs `v1`) when variants named `full` and
  `full-<component>` exist. Seeds are averaged per series before every test.
