# Research: Motif quality

Notation: series `x[0..n)`; a set `M = {(sᵢ, lᵢ)}ᵢ₌₁..k`; `L* = round(mean lᵢ)`; `vᵢ` = occurrence i
resampled to `L*` (and z-normalised for z-distances); `D` = selected distance (RMS-normalised as
today); `ρ = max_stretch`.

## Evidence from `results/bench-001` and the run results

| Symptom | Evidence | Root cause |
|---|---|---|
| Low precision | synthetic: 46 found vs 18 true occurrences per series, TP 419 / FP 2 344 | support objective without radius; cohesion = mean, barely penalises one weak member |
| Biased cohesion | `adm-daily-long`: sets with k = 2 and cohesion 0.05 | member included in its own prototype (distance halved for k = 2) |
| Length at the bound | electric / adm-daily: almost every occurrence = `max_len` | "maximise mean length" objective |
| Junk ranked first | `adm-daily-long` motif #1: tiling of the whole series, cohesion 0.96 | top-K by distance to the ideal point; normalised support dominates |
| Duplicates | electric motifs 1 and 3 share the same shape | dedupe by coverage Jaccard only |
| Early stop | stagnation after 45–207 generations, < 1 s | best support / length saturated at generation 0; noisy HV after clearing |
| DTW on levels | `Distance::znorm()` true only for ZEuclid | no per-window normalisation for DTW; mean prototype for DTW |
| Benchmark | STOMP n_sets = 1.0 everywhere; LoCoMotif TP = 0 on synthetic; ditscover `max_stretch` 1.5 < GT 1.86 | `stumpy.motifs` default `cutoff`; LoCoMotif parameters / lengths; untuned params |

## R1. Benchmark fixes

- **STOMP**: `stumpy.motifs(..., cutoff=np.inf, max_motifs=k, max_matches=max_occ)`; keep the
  library's default `max_distance` (that is its radius, part of the method). Expose `cutoff` and
  `max_distance` as tunable params. Check (T010): two planted shapes × 4 occurrences come back as
  two exact sets with `max_matches = 4`; with 20 the default radius merges both shapes into one
  set, so the grid is `max_matches ∈ {5, 10, 20}`.
- **LoCoMotif**: spike on stored outputs: check `(b, e)` semantics, distribution of found lengths vs
  GT, and `l_min`/`l_max` meaning (representative length). Outcome below ("LoCoMotif diagnosis").
- **LoCoMotif diagnosis** (T002, 3 synthetic series, n = 5000, GT lengths 28–102): the adapter's
  coordinates are right (`(b, e)` with `e` exclusive → `(b, e − b)`). LoCoMotif compares raw
  values, so the low-amplitude background windows are mutually similar and each planted
  occurrence sits on a different random-walk level: with `rho ∈ {0.8, 0.9, 0.95}` it returns 2–40
  background fragments of 13–57 points per set (F1 = 0 for every length window tried, warping on or
  off). A moving-average high-pass (window 2·max GT length) plus `rho = 0.6` gives F1 0.09–0.17;
  `rho = 0.6` alone with the GT length window 0.03–0.17. Fix: no coordinate change; the adapter gains
  `highpass` (subtract a moving average of width 2·`max_len`) and the tuning grid is
  `rho ∈ {0.5, 0.6, 0.7, 0.8} × highpass ∈ {false, true}` (8 combinations, warping on). The low
  synthetic score is a property of the method on level-shifted motifs, reported as such. Its
  pointwise similarity rates flat or noisy stretches as near-identical (0.997 at noise 0.02), so on
  a noise or random-walk background it prefers background "motifs" even for clean planted sines;
  on a TSMD-like series (distinct shapes around the repeats) it hits ≥ 3 of 4 planted occurrences
  with the right coordinates (`test_locomotif_hits_a_planted_motif_with_matching_coordinates`).
- **Length / stretch window** given to all methods: `min_len = ⌊min GT len·(1 − b)⌋`,
  `max_len = ⌈max GT len·(1 + b)⌉` (as today, `b = 0.2`) and, new, `max_stretch = max over GT sets
  of (max len / min len) · 1.1`, applied to ditscover (LoCoMotif warps; STOMP is fixed-length by
  design).
- **Tuning**: `tune: {n_series, metric: f1}` + per-method `grid: {param: [values]}`; validation
  split = TSMD `validation`, synthetic = same generator with `seed + 10⁶`. Best mean F1 wins (ties →
  first in grid order); chosen params stored in `bench.yaml`. Guard: `|grid| · n_series · seeds`
  estimated before running; refuse above `max_runs`.
- **Stored sets**: `sets.jsonl`, one line per run key with the found sets.
- **Statistics**: paired Wilcoxon signed-rank on per-series F1 (seeds averaged first), per dataset
  and pair of methods, two-sided, `scipy.stats.wilcoxon` (`zero_method="zsplit"`); Holm correction
  across pairs. `scipy` is already a dependency of `stumpy` (bench extra).
- **Error view**: per series, one row per method: GT occurrences as grey bands, found occurrences
  green (hit, IoU > 0.5 with an unmatched GT occurrence under the PROM assignment) or red (false
  alarm), missed GT outlined. Max 20 series per dataset in the report.

## Corrected baseline (T019, `results/bench-002`)

`contracts/baseline.example.yaml`: previous ditscover (`v1` row, stretch window fixed), tuned STOMP
and LoCoMotif, test split, 20 series per dataset, 3 seeds for ditscover; 1 305 runs, 0 failures;
stored sets, Errors tab and paired tests present. Tuned on validation: STOMP `max_matches` 5
(10 on ECG5000 / Symbols); LoCoMotif `rho` 0.7–0.8 without high-pass on TSMD, `rho = 0.5` with
high-pass on synthetic.

| Dataset | v1 P / R / F1 | STOMP P / R / F1 | LoCoMotif P / R / F1 |
|---|---|---|---|
| synth-varlen | 0.16 / 0.39 / 0.22 | 0.93 / 0.39 / 0.53 | 0.05 / 0.17 / 0.08 |
| ECG5000 | 0.47 / 0.40 / 0.41 | 0.55 / 0.55 / 0.54 | 0.63 / 0.56 / 0.57 |
| Fungi | 0.27 / 0.48 / 0.33 | 0.84 / 0.55 / 0.65 | 0.04 / 0.05 / 0.05 |
| Plane | 0.68 / 0.60 / 0.61 | 0.86 / 0.65 / 0.74 | 0.86 / 0.94 / 0.89 |
| Mallat | 0.34 / 0.47 / 0.37 | 0.37 / 0.27 / 0.31 | 0.60 / 0.68 / 0.63 |
| Symbols | 0.54 / 0.52 / 0.52 | 0.63 / 0.54 / 0.57 | 0.72 / 0.86 / 0.77 |

Paired Wilcoxon (Holm within dataset): v1 is significantly below the best baseline on every
dataset (p_holm ≤ 0.001) except ECG5000 vs STOMP (0.097), Mallat vs STOMP (0.62) and Symbols vs
STOMP (0.31). Median time at 20 000 points: v1 3.1 s, STOMP 0.65 s, LoCoMotif 67.8 s. With the
cut-off removed STOMP is the strongest baseline on synthetic data and Fungi (precision 0.93 / 0.84);
LoCoMotif leads on Plane, Mallat and Symbols. SC-002 / SC-003 are judged against these numbers.

## R2. Fitness v2

### Member distances (leave-one-out)

Euclidean-type distances (Euclid, ZEuclid): `p = (1/k) Σ vᵢ`, `p₋ᵢ = (k·p − vᵢ)/(k − 1)`,
`dᵢ = D(vᵢ, p₋ᵢ)` — O(k·L*), no extra pass. DTW-type (Dtw, ZDtw): pairwise matrix
`Δᵢⱼ = D(vᵢ, vⱼ)` (k ≤ 20 ⇒ ≤ 190 pairs), medoid `m = argminᵢ Σⱼ Δᵢⱼ`; `dᵢ = Δᵢₘ` for `i ≠ m`,
`dₘ = min_{j≠m} Δₘⱼ`.

**Extent** `E = quantile_q(d₁..dₖ)`, `q = extent_q` (0.9; with k ≤ 10 it is the max). **Cohesion**
`C = mean(dᵢ)` is kept for reporting.

### Random-pair scale

`δ(L)` = median `D` between 256 random pairs of non-overlapping, non-flat windows of length `L`,
computed once per run on a grid of 8 lengths spanning `[min_len, max_len]` (linear interpolation in
between). Seeded RNG ⇒ deterministic.

### Nearest outsider and margin

`d_out = min D(w, p)` over windows `w` with lengths `L*·{ρ^-½, 1, ρ^½}` that do not overlap any
occurrence (with `min_gap`). Margin `μ = (d_out − E) / δ(L*)`.

Cost control — **candidate index** (shared with `add`, seeding, immigrants):
- For each length `ℓ` of a grid (8 values over `[min_len·ρ^-½, max_len·ρ^½]`), sketch every window
  starting at stride `s = max(1, ⌊ℓ/8⌋)`: z-normalise, PAA to 16 values. Memory: n/s·16·8 floats
  (20 000 points ⇒ ≈ 1 MB; measured 1.56 MB with lengths 40–135 and ρ = 2, bound 2 MB).
- Query: PAA of the prototype → `LB = RMS distance between PAA vectors`. For windows of length
  `L*` and Euclid-type distances this is a true lower bound of the RMS distance (Keogh et al.,
  2001), so scanning in LB order and stopping when `LB ≥ best` is exact. For other lengths
  (resampling) and DTW it is used as a filter: exact distances on the 32 best candidates. Refine the
  winner with a step-1 search in `±s/2`. Sketches are z-normalised only for z-distances.
- Expected cost per evaluation: ~3·n/s·16 flops + few exact distances ⇒ ~10⁵ flops for n = 20 000.

### Objectives (all minimised)

| | Value | Normalised |
|---|---|---|
| f1 | closed coverage `Σ lᵢ / n · w(μ)` (maximise) | `1 − min(1, coverage / coverage_max) · w(μ)`, `coverage_max = max_occ·max_len/n` capped at 1, `w(μ) = clamp(μ / 0.5, 0, 1)` |
| f2 | extent `E / δ(L*)` (minimise) | clamp to `[0, 1.5]` then `/1.5` |
| f3 | contrast `(d_out − E) / δ(L*)` (maximise) | `(1 − clamp(·, −1, 1)) / 2` |

**Relative margin** (implementation decision, T024): `μ = (d_out − E) / max(d_out, E)` ∈ [−1, 1],
same sign as the margin of the glossary. Evidence (synthetic series #0, v2 run before the change):
the planted sets have `E/δ` 0.08–0.20, `d_out/E` 2.1–2.3, δ-normalised margin 0.11–0.21; the
bloated sets the run returned (k = 13–20, coverage 0.15–0.36) have `E/δ` 0.29–0.51, `d_out/E`
0.99–1.10, δ-normalised margin −0.004–0.05. A set that covers a third of the series is "closed by
elimination" (few non-overlapping windows remain as outsiders), and `coverage · margin_δ` then
ranks bloat at par with the true sets (0.016 vs 0.010). With the relative margin the planted sets
score μ 0.52–0.57 vs 0.02–0.09 and quality 0.025–0.052 vs ≤ 0.013, and they dominate the bloated
sets on all three objectives. f3 keeps the δ-normalised contrast: it is monotone under far
additions (the proof behind `far_additions_never_improve_extent_or_margin`), the relative form is
not when μ < 0. Gating f1 by `w(μ)` makes "true set + junk" worse on f1 too (μ < 0 ⇒ no coverage
credit), as US2-1 requires; plain coverage would improve.

Why it fixes the symptoms: adding a weak occurrence raises `E` (f2 worse) and lowers `μ` (f3 worse);
missing a true occurrence leaves it as nearest outsider (low `d_out`, f3 worse); a tiling of the
series has `E/δ ≈ 1` and `μ ≤ 0`; length no longer has its own objective, so occurrences grow only
while they stay closed. The two-occurrence bias disappears with leave-one-out.

### Quality score (ranking, labelling weights)

`Q = √coverage · max(μ, 0) · max(1 − E/δ, 0)` with the relative margin μ — zero for sets that are
not closed. Implementation decision (T052 calibration, validation split, 5 series per dataset,
seed 0): the first form `coverage · max(μ, 0)` ranked a loose tiling first on Plane (10
instance-aligned windows of several classes covering 74 % of the series: E/δ 0.46, μ 0.62,
Q 0.46) above the planted 3-occurrence sets (E/δ 0.22, μ 0.48–0.63, Q 0.13–0.17); the tiling is
genuinely closed, only looser. Adding tightness `1 − E/δ` and damping coverage moved the sample
mean F1 from 0.451 to 0.587 (coverage^¼) / 0.585 (coverage^½): Plane 0.35 → 0.83 / 0.89, ECG5000
0.45 → 0.69 / 0.71, Mallat 0.38 → 0.52 / 0.43, Symbols 0.59 → 0.65, Fungi 0.36 (unchanged),
synthetic 0.58 → 0.47 / 0.48. The two exponents tie; √ is kept. Configurable `rank_by: quality |
ideal` (ideal = current behaviour) for the ablation.

### Feasibility and complexity

- `min_std_rel` (0.05): a window is infeasible if its std < 0.05 × the median std of the `min_len`
  windows (`min_std` kept as absolute floor). The reference is the median window std, not the
  series std: on a random walk the global std grows with n and flagged most short windows as flat
  (the feature-001 invariants proptest could no longer initialise).
- `cid` (false): complexity-invariant distance (Batista et al., 2014),
  `D_CID(a, b) = D(a, b) · max(CE(a), CE(b)) / min(CE(a), CE(b))`, `CE(a) = sqrt(Σ (aᵢ₊₁ − aᵢ)²)`.

### Distances

- `zdtw`: z-normalise each resampled window, then Sakoe-Chiba DTW (band as today). Prototype for DTW
  kinds = medoid (above). Existing `dtw` unchanged for compatibility.

### Alternatives considered

- MDL "bits saved" single objective (principled, but heavier and harder to interpret; later feature
  if needed).
- Silhouette over all windows (needs all-pairs distances; O(n²)).
- Fixed radius `r` as a gene (adds a parameter the margin already captures adaptively).

## R3. Search

- **Boundary refinement** (`refine`, p = 0.3 per offspring; always for archive entrants —
  Lamarckian): coordinate descent, 2 passes, per occurrence `start ± ⌈0.1·l⌉` (step 1) then
  `len · {0.95, 1.05}` around the centre, minimising `dᵢ`; repair afterwards. Cost O(k·0.2·l·L*).
- **Collective moves**: `phase` (p = 0.2): `δ ~ round(N(0, σ_shift·L̄))` applied to every start;
  `rescale` (p = 0.15): `f = exp(N(0, σ_stretch))` applied to every length around each centre.
- **Non-ratcheting variation**:
  - `add`: best outsider from the candidate index accepted iff the new extent ≤ current extent.
  - `remove`: drop `argmax dᵢ` (with p = 0.5 random as today only under `fitness: v1`); accept iff
    `μ` increases.
  - donation (crossover): `b` accepted iff `D(b, p_A) ≤ E_A`.
  - **mating restriction**: second parent drawn by tournament among individuals with containment
    > 0 or prototype shape distance < `mating_tau·δ`; if none, the pair is only mutated.
- **Split** (p = 0.05, k ≥ 2·min_occ): 2-medoids on `Δ`; if it reduces the total member distance by
  ≥ 30 %, the larger group (if ≥ min_occ) replaces the child.
- **Nearest-neighbour seeding** (`seed_nn`, share 0.4 of the initial population): from the
  candidate index, take the 4·N windows with the smallest nearest-neighbour distance (approximate
  matrix profile over the sketch grid); each seed is grown with its nearest non-overlapping
  windows sorted by distance and cut at the largest ratio `d₍ⱼ₊₁₎/d₍ⱼ₎` for `j ∈ [min_occ, max_occ]`.
  Remaining share: current greedy and random initialisers.
- **Archive**: bounded (`archive`, 2N) set of distinct non-dominated motifs (dominance in v2
  objectives, distinctness = FR-024 rule); updated every generation; HV and stagnation computed on
  it; final selection from archive ∪ population.
- **Stopping**: `min_generations` (100), stagnation window default 50, `stagnation_delta` relative
  (1e-3 of archive HV).
- **Immigrants** (`immigrants`, 0.1): when the archive HV did not improve for half a window, replace
  the worst 10 % by NN seeds whose windows are not covered by any archive member.

### Implementation notes (T033–T039)

- **Refinement**: shift-only passes until no move (≤ 4), then resize passes (≤ 4) trying the
  fixed start, fixed end and fixed centre. Resizing while the others are still misaligned shrank
  every occurrence of a noise-free planted set to a consistent 56-point sub-window (identical
  shapes make any consistent sub-window a perfect match); centre-only resizing could not fix an
  end once the start was right. The result is kept only if the mean squared member distance does
  not grow.
- **Gap cut**: each candidate window is first aligned by a step-1 search within ±stride/2;
  strided windows sit up to half a stride off a true occurrence, which put the largest distance
  jump inside the planted group (3 of 6 kept).
- **Split**: accepted when the mean distance per non-medoid member drops by ≥ 30 % (two medoids
  absorb two members, so the plain total drops by ~1/3 even for one shape with k = 4).
- **Mating restriction**: shape distance between z-normalised prototypes resampled to 32 points,
  threshold `mating_tau · √2` (√2 = random-pair distance of z-normalised shapes); up to 8 random
  candidates, best of the first two compatible.
- **Archive**: a candidate that overlaps members (containment > `dedupe_overlap`) enters only if
  it dominates all of them, so the archive HV never drops unless the bound evicts (crowding).
  Shape-based dedupe is left to the final selection (R4).
- **NN seeding**: 256 query windows per level (512 made start-up dominate short runs).
- **Stopping**: a cancel or an elapsed time budget stops starting new offspring pairs, so both
  land within one evaluation (heavier v2 generations made between-generation checks too coarse).

## R4. Selection and labelling

- **Ranking**: archive ∪ population sorted by `Q` (desc), ties by rank then index.
- **Dedupe**: candidate discarded if, for a better chosen motif, containment
  `|A ∩ B| / min(|A|, |B|) > dedupe_overlap` (0.5) or shape distance `< dedupe_shape·δ` (0.3), where
  shape distance = min over lags `|τ| ≤ 0.25·L` of RMS z-distance between prototypes resampled to a
  common length on the overlapping part.
- **Labelling**: all occurrences of the chosen motifs as weighted intervals, weight
  `wᵢ = max(d_out − dᵢ, ε) / δ` (how clearly the occurrence belongs to its motif); maximum-weight
  set of non-overlapping intervals by the classic DP (sort by end, binary search predecessor),
  O(N log N). Motifs left with < min_occ occurrences are dropped and the next candidate is taken;
  repeat until K motifs or candidates exhausted. Labels: `Vec<i32>` over original indices, `-1` none.
- `top_k: auto`: candidates with `μ > 0` only (checked again after the conflicts trimmed them).
- Implementation (T044): a motif the conflicts trimmed is re-evaluated and treated as starved when
  it is no longer closed (`μ ≤ 0`), so the next candidate takes its place. Without it the third
  motif of synthetic series #0 kept two leftover windows with μ = −0.24; with it the quick
  5-series check went from P/R/F1 0.42/0.42/0.42 to 0.60/0.52/0.56 (v1: 0.16/0.40/0.23).

## R5. Ablation protocol

- Variants (benchmark methods with labels): `v1` (feature 001 defaults with the R1 stretch fix),
  `full`, and `full − X` for X ∈ {loo, margin, coverage, scale, refine, collective, guarded_add,
  mating, split, seed_nn, archive, immigrants, quality_rank, dedupe_v2, min_std_rel, labelling}.
  `medoid`, `zdtw` and `cid` are used only when DTW or CID is selected and stay outside the
  ablation. `labelling` is measured but never deleted (output guarantee, FR-025 / SC-004).
- **Sample size** — Decision: 30 series per dataset and split, 3 seeds averaged per series before
  testing. Rationale: with n = 10 the smallest two-sided exact Wilcoxon p is 2/2¹⁰ ≈ 0.002, above
  the first Holm threshold 0.05/15 ≈ 0.0033 only when all 10 series agree, so the strict rule would
  delete almost everything; n = 30 detects moderate effects. Alternatives: 10 series (too weak);
  (series, seed) pairs as samples (not independent, inflates significance).
- **Splits** — Decision: the ablation runs on validation and test; decisions use validation only;
  test ΔF1 is reported. Datasets are identified by `(name, split)`; the synthetic validation split
  is the same generator with `seed + 10⁶`. Cost: 18 variants × 6 datasets × 2 splits × 30 series ×
  3 seeds = 19 440 runs × ~2 s ≈ 11 h sequential (overnight; `jobs` shortens it on many cores).
  Baselines are not part of the ablation config (they do not depend on components).
- **Test and correction** — Decision: paired Wilcoxon signed-rank (`zero_method="zsplit"`, two-sided)
  of per-series F1, `full` vs `full − X`; Holm across the 15 components within each
  `(dataset, split)`. Alternative rejected: Holm across components × datasets (90 tests; threshold
  0.05/90 unreachable even at n = 30 for small effects).
- **Decision rule** (validation only) — keep X iff removing it lowers F1 with Holm p < 0.05 on ≥ 1
  dataset and raises it with Holm p < 0.05 on none; every other component (including no significant
  effect) is deleted with its switch, config leaf and tests. Alternative rejected: keep neutral
  components that cost < 20 % time (dead weight under Principle III).
- **Joint check** — pruned `full` is compared with the unpruned `full` on validation; if it is
  significantly worse on any dataset, the deleted component with the largest ΔF1 is restored and
  the check repeated (one-at-a-time removal misses interactions).
- **v1** — after pruning, `full` vs `v1` is tested with the same rule; if `full` wins, `model: v1`,
  `rank_by: ideal`, `dedupe_jaccard` and the v1-only branches (random `remove`, support / length
  objectives) are deleted; `results/bench-002` and the ablation `v1` rows remain the recorded
  baseline. If `full` does not win, the v2 components are deleted instead and the outcome recorded.
- **Final evaluation**: pruned `full` vs tuned baselines once on test (separate config,
  `contracts/final.example.yaml`) for SC-001..SC-003.
- Report: ablation tab with ΔF1 (mean, 95 % bootstrap CI) per component × dataset × split, marks for
  significance, and the keep / delete decision per component.

## R7. Run-time target (SC-005)

- Decision: measured with library defaults (including `min_generations`), one seed, all cores, on
  a 20 000-point synthetic series with planted motifs; median of 5 seeds < 10 s. A `criterion`
  bench (`v2_defaults_20k`) tracks it and a slow-marked pytest asserts it in release builds.
- Consequence: default `population` and `min_generations` are sized so the floor (100 generations)
  fits the budget; if it does not, `min_generations` is lowered before any other default.
- Alternative rejected: a `max_time` stop criterion (hides the cost instead of bounding it).
- Measured (T050, M-series laptop, 10 cores): synthetic 20 000 points, lengths 40–135, defaults:
  median of 5 seeds 2.33 s (`test_perf.py`); criterion `v2_defaults_5k` 2.03 s,
  `v2_defaults_20k` 3.26 s. No default needed lowering.
- Long motifs: the first Mallat probe (motifs ≈ 1 000 points, threads 2) took 144–163 s per run
  (v1: 4.2 s, LoCoMotif: 14.8 s), 85 % in refinement. Two caps make the cost independent of the
  motif length: local searches (refinement moves, outsider and seed alignment) probe ~16 evenly
  spaced positions then step 1 around the best (`index::coarse_fine`; exhaustive for ranges ≤ 16,
  so short motifs are unchanged), and v2 compares occurrences resampled to at most 256 points
  (`motif::EVAL_RES`; refinement 128). Mallat: 5.7–6.1 s per run, same F1 (0.25 / 0.32 on the two
  probe series); the 5-series synthetic check is unchanged (F1 0.56). v1 keeps full resolution.
- After pruning (T057): `test_perf.py` passes (5 seeds in 10.3 s total); criterion
  `v2_defaults_5k` 1.56 s (−23 %), `v2_defaults_20k` 1.76 s (−46 %). Against `pre-003` the
  feature-001 benches are 68–148 % slower (plan.md Complexity Tracking). Per-dataset median time
  vs LoCoMotif: R9.
- Benchmark worker: ditscover's warm-up run is one generation of population 4 (it has no JIT to
  warm; the full warm-up doubled every job).

## R8. Ablation decisions

Study: `results/ablation-003` (19 440 runs, 0 failures; 30 series × 3 seeds per dataset and
split). ΔF1 = F1(`full − X`) − F1(`full`), seeds averaged per series; bold = Holm p < 0.05 (p in
brackets). For `v1` the row is F1(`v1`) − F1(`full`).

Validation (decisions):

| Removed | synth | ECG5000 | Fungi | Plane | Mallat | Symbols | Decision |
|---|---|---|---|---|---|---|---|
| v1 (instead of full) | **−0.396** (<0.001) | **−0.203** (0.002) | +0.056 | **−0.201** (<0.001) | −0.029 | **−0.243** (<0.001) | full wins |
| loo | −0.042 | −0.008 | +0.003 | −0.050 | −0.007 | −0.051 | delete |
| margin | +0.042 | +0.016 | −0.017 | +0.017 | +0.009 | −0.011 | delete |
| coverage | +0.033 | +0.032 | −0.012 | −0.020 | −0.019 | +0.001 | delete |
| scale | +0.051 | −0.018 | −0.002 | −0.019 | 0.000 | 0.000 | delete |
| refine | −0.057 | −0.032 | +0.027 | −0.005 | +0.026 | −0.046 | delete |
| collective | +0.008 | −0.013 | +0.009 | +0.003 | −0.006 | +0.012 | delete |
| guarded_add | −0.052 | −0.020 | −0.016 | +0.011 | +0.018 | +0.005 | delete |
| mating | +0.062 | −0.006 | −0.026 | +0.014 | +0.011 | +0.011 | delete |
| split | +0.005 | −0.005 | −0.005 | +0.016 | +0.006 | +0.023 | delete |
| seed_nn | −0.024 | +0.001 | +0.027 | −0.008 | −0.029 | +0.014 | delete |
| archive | +0.002 | −0.003 | −0.004 | −0.012 | −0.005 | −0.004 | delete |
| immigrants | −0.029 | −0.005 | −0.001 | −0.006 | 0.000 | +0.010 | delete |
| quality_rank | **−0.104** (0.003) | −0.086 | −0.012 | **−0.388** (<0.001) | −0.021 | **−0.205** (0.037) | keep |
| dedupe_v2 | −0.033 | +0.037 | −0.010 | **−0.145** (0.019) | +0.042 | −0.003 | keep |
| min_std_rel | 0.000 | 0.000 | +0.010 | 0.000 | 0.000 | 0.000 | delete |
| labelling | −0.028 | +0.056 | **+0.105** (<0.001) | +0.020 | +0.012 | +0.024 | keep (FR-025) |

Test ΔF1 that disagree with the validation decision (reported, not used):

- `refine`: removal lowers synth F1 by **0.176** (p < 0.001) and ECG5000 by 0.076 (n.s.).
- `loo`: removal lowers synth F1 by **0.081** (p = 0.038).
- `dedupe_v2`: removal raises Mallat F1 by **0.061** (p = 0.005); kept on validation by Plane.
- `labelling`: removing it raises F1 on Fungi on both splits (validation significant): conflict-free
  trimming drops correct but overlapping occurrences. It stays as an output guarantee.

Most switches have small effects of mixed sign, so one-at-a-time removal is underpowered for them;
the joint check below decides whether removing them together is safe.

**Joint check** (validation, same 30 series × 3 seeds, Wilcoxon vs the unpruned `full` and vs `v1`
from the study, Holm over the two comparisons). "Largest ΔF1" = the deleted component whose removal
cost the most F1 on the dataset(s) where the pruned variant fails.

| Step | Restored | Fails vs `full` (ΔF1, Holm p) |
|---|---|---|
| 1 | — (all 13 off) | synth −0.308 (<0.001) |
| 2 | refine | synth −0.129 (<0.001), Symbols −0.062 (0.023) |
| 3 | + guarded_add | synth −0.163 (<0.001), Symbols −0.073 (0.022) |
| 4 | + loo | Mallat −0.060 (0.041) |
| 5 | + seed_nn | none |

Step 5 vs `full`: synth −0.011, ECG5000 −0.034, Fungi −0.029, Plane +0.008, Mallat +0.031,
Symbols −0.038 (all Holm p ≥ 0.10). Run logs: scratch configs `joint*.yaml`, not kept in the repo.

Final component set — **kept**: loo, refine, guarded_add, seed_nn, quality_rank, dedupe_v2,
labelling. **Deleted**: margin objective (f3 = cohesion / c_ref), coverage objective (f1 = support
objective), null scale in the objectives (f2 / f3 use `c_ref`; δ stays in `quality`), collective
moves (phase, rescale), mating restriction, split, archive, immigrants, `min_std_rel`.

**v1 decision**: pruned `full` vs `v1` on validation: synth +0.385 (<0.001), ECG5000 +0.168
(0.008), Plane +0.209 (<0.001), Symbols +0.205 (<0.001), Fungi −0.085 (0.069), Mallat +0.059
(0.43). `full` wins (better on 4, worse on none): `model: v1`, `rank_by: ideal`,
`dedupe_jaccard`, the length objective and the random `remove` are deleted (T055);
`results/bench-002` and the ablation `v1` rows remain the recorded feature-001 baseline.

Implementation notes:

- The joint check ran on the switches (same code as the study) before any code was deleted, so a
  restore cost no code churn; deletion followed once step 5 passed. Every deleted switch was a
  no-op at its "off" value (`chance(0)` draws nothing), and the pruned build reproduces the old
  build with those switches off run for run (checked on 7 benchmark runs: identical F1).
- Kept components keep their ablation switches (`loo`, `refine`, `guarded_add`, `seed_nn`,
  `dedupe_overlap` / `dedupe_shape`, `labels`); the unguarded add / remove / donate path is the
  `guarded_add: false` branch, not v1 code, so it stays. `rank_by: ideal` (the `quality_rank`
  switch) was deleted with v1 as T055 prescribes, so `contracts/ablation.example.yaml` lost that row.
- `crates/core/tests/planted.rs` (feature 001 SC-001, release builds only; debug `cargo test`
  ignores it) does not pass on this scenario (10 000 points, 3 motifs × 8 occurrences, lengths
  40–135): median recall / precision 0.542 / 0.188 on `main`, 0.646 / 0.698 before pruning,
  0.625 / 0.626 after (thresholds 0.9 / 0.8). It was already failing before this feature; the
  threshold is left unchanged and the gap is reported.

## R9. Final benchmark (test split)

Run: `contracts/final.example.yaml` → `results/bench-003` (1 260 runs, 0 failures; 20 test series
per dataset, ditscover × 3 seeds, baselines tuned on validation). Mean F1 ± std, median time per
run; Wilcoxon over series (seeds averaged), Holm per dataset.

| Dataset | STOMP | LoCoMotif | ditscover | ditscover time s (LoCoMotif) |
|---|---|---|---|---|
| synth | 0.532 ± 0.149 | 0.081 ± 0.072 | **0.606 ± 0.241** | 0.96 (1.66) |
| ECG5000 | 0.542 ± 0.293 | 0.574 ± 0.209 | **0.584 ± 0.203** | 0.46 (0.04) |
| Fungi | **0.651 ± 0.220** | 0.046 ± 0.073 | 0.307 ± 0.325 | 1.18 (1.13) |
| Plane | 0.740 ± 0.123 | **0.892 ± 0.111** | 0.873 ± 0.157 | 0.69 (0.34) |
| Mallat | 0.312 ± 0.277 | **0.635 ± 0.242** | 0.353 ± 0.267 | 1.10 (5.33) |
| Symbols | 0.570 ± 0.324 | **0.775 ± 0.175** | 0.767 ± 0.189 | 0.69 (0.53) |

Best mean rank overall: ditscover (1.89; LoCoMotif 2.03, STOMP 2.08).

| Criterion | Outcome |
|---|---|
| SC-001 | **Not met.** Synthetic test median F1 0.603, median precision 0.667 (target ≥ 0.70 each; feature 001: 0.22 / 0.15). |
| SC-002 | **Not met.** Significantly lower than STOMP on Fungi (Holm p < 0.001) and LoCoMotif on Mallat (p < 0.001); not significantly different elsewhere. On synth it beats LoCoMotif (p < 0.001) but not STOMP (p = 0.18). |
| SC-003 | **Met.** Symbols 0.767 ≥ 0.75; ECG5000 0.584 ≥ STOMP 0.542. |
| SC-004 | **Met.** 0 of 360 ditscover runs report overlapping occurrences across their motifs. |
| SC-005 (run time vs LoCoMotif) | **Partly met.** Faster on synth and Mallat; slower on ECG5000, Fungi, Plane, Symbols (short series where LoCoMotif needs well under a second). The 20 000-point target holds (R7, T057). |
| SC-006 | **Met** on validation (R8); the test split disagrees for `dedupe_v2` on Mallat (R8). |

Reading: two datasets account for the SC-002 miss. Fungi: conflict-free labelling drops correct but
overlapping occurrences (removing it raised Fungi F1 by 0.105 in the ablation). Mallat (motifs
≈ 1 000 points): not investigated; the evaluation resolution cap (`EVAL_RES`, R7) did not change F1
on the two probe series, so it is not the evident cause. Both are left for a later feature; the
targets are unchanged.

## R6. Dependencies

- Rust: none new (PAA, DP, 2-medoids, bootstrap are a few lines each).
- Python bench extra: `scipy` (already transitive via `stumpy`; declared explicitly).
