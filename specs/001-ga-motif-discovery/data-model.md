# Data Model

Core types (`crates/core`). All `serde::{Serialize, Deserialize}` unless noted.

## Series

| Field | Type | Rule |
|---|---|---|
| `values` | `Vec<f64>` | no NaN after preprocessing; `len ≥ 2·min_len` |
| `origin` | `Vec<u32>` | original index of each point (increasing); `origin.len() == values.len()` |
| `scale` | `u32` | accumulated PAA factor (1 by default) |

Derived (not serialised): `Stats { sum: Vec<f64>, sum2: Vec<f64> }` prefix sums for O(1)
mean/std. Mapping to original: `(origin[s], origin[s+len−1] + scale − origin[s])`.

## Step (preprocessing extension point)

`enum Step { Nan(Fill), Detrend(Trend), Smooth(usize), Normalize(Norm), Paa(usize) }` with
`Fill = Interpolate | Ffill | Drop`, `Trend = Linear | Diff`, `Norm = Zscore | Minmax`.
Applied in declared order; each step is checked against the current length before running (R19).

## Segment

`{ start: u32, len: u32 }` — `start + len ≤ n`, `min_len ≤ len ≤ max_len`.

## Individual

| Field | Type |
|---|---|
| `segs` | `Vec<Segment>` sorted by `start`, non-overlapping |
| `obj` | `[f64; 3]` normalised (minimise): support, cohesion, length |
| `raw` | `Raw { support: u32, cohesion: f64, mean_len: f64, stretch: f64 }` |
| `rank` | `u32` (not serialised) |
| `crowd` | `f64` (not serialised) |

Invariants (proptest): R2 of research.md after any operator.

## Scratch (not serialised)

`{ a: Vec<f64>, b: Vec<f64>, row: Vec<f64> }` sized to `max_len`, one per Rayon worker (R17).

## Motif (output)

| Field | Type |
|---|---|
| `occurrences` | `Vec<(u32, u32)>` in original indices (start, length) |
| `prototype` | `Vec<f64>` of length `L*` |
| `aligned` | `Option<Vec<Vec<f64>>>` occurrences resampled (and z-normalised if the distance is `zeuclid`) to `L*`; `Some` only for top-K motifs, `None` in `front` |
| `support`, `cohesion`, `mean_len`, `stretch` | from `Raw` |
| `objectives` | `[f64; 3]` normalised |

## Config

Tree with `deny_unknown_fields`; defaults in brackets. Leaf names are unique (allows flat
overrides).

- `seed: u64` (0)
- `data`: `path: Option<PathBuf>`, `column: Column = Index(0) | Name(String)`, `delimiter: char` (`,`),
  `header: bool` (true)
- `preprocess: Vec<Step>` ([]), YAML form: `nan: interpolate|ffill|drop` · `detrend: linear|diff` ·
  `smooth: usize (≥2)` · `normalize: zscore|minmax` · `paa: usize (≥2)`
- `motif`: `min_len` (required), `max_len` (required, ≥ min_len), `max_stretch` (1.5, ≥ 1),
  `min_occ` (2, ≥ 2), `max_occ` (20, ≥ min_occ), `min_gap` (0), `min_std` (1e-6)
- `distance`: `kind: zeuclid|euclid|dtw` (zeuclid), `band: f64 ∈ (0,1]` (0.1, DTW only) → mapped to
  the core `enum Distance`
- `ga`: `population` (200, even ≥ 4), `crossover` (0.9), `shift` (0.3), `stretch` (0.2), `add` (0.2),
  `remove` (0.1), `sigma_shift` (0.1), `sigma_stretch` (0.1), `scan_step` (0.125, scan stride as a fraction of the window length, in
  (0,1]), `init_greedy` (0.5),
  `niche_jaccard` (0.5) — probabilities in [0,1]
- `stop`: `max_generations` (300), `time_budget_s: Option<f64>` (none), `stagnation_window:
  Option<u32>` (30; `null` disables), `stagnation_delta` (1e-4)
- `output`: `dir: Option<PathBuf>` (none in the API; the CLI defaults it to `./ditscover-out`),
  `top_k` (5), `dedupe_jaccard` (0.3), `snapshot_every` (10)
- `log`: `level` (info), `log_every` (10), `history: bool` (true), `channel_capacity` (1024)

Validation (R19), all in `config.rs`, before the engine starts; errors carry the key path
(`motif.max_len: must be >= min_len (12)`, `preprocess[1].smooth: window 50 > series length 40`):

1. `Config::validate()` — config only.
2. Per-step check inside `preprocess::apply` — step vs current length.
3. `validate_len(n, &Config)` — `n ≥ 2·min_len` after preprocessing.

## Event (channel)

`enum Event { Start { config, version }, Generation(GenerationRecord), Finish { reason, dropped } }`

## GenerationRecord

`{ gen, elapsed_s, evals, hv, front_size, best: [f64;3], mean: [f64;3], front: Option<Vec<[f64;3]>> }`
— `best`/`mean` in raw units (support, cohesion, length). `front` every `snapshot_every`.

## RunResult

`{ motifs: Vec<Motif>, front: Vec<Motif>, history: Vec<GenerationRecord>, stop: StopReason,
generations, elapsed_s, dropped_events: u64, config: Config, version: String, series: Vec<f64> }`

- `motifs` sorted by (Pareto rank, distance to ideal point) — FR-015.
- `series`: original series before preprocessing (NaN allowed). Saved to `series.csv`, never in
  `results.json` (R18).

`StopReason = MaxGenerations | TimeBudget | Stagnation | Cancelled`

## Run states

`Validated (config) → Prepared (loaded, preprocessed, length checked) → Running →
(MaxGenerations | TimeBudget | Stagnation | Cancelled) → Finished`.
Cancelled before generation 0 completes ⇒ `motifs = []`.
