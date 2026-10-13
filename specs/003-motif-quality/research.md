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
  `max_distance` as tunable params.
- **LoCoMotif**: spike on stored outputs: check `(b, e)` semantics, distribution of found lengths vs
  GT, and `l_min`/`l_max` meaning (representative length). Hypothesis: `l_max` ≈ 125 lets warped
  occurrences span more than twice a true occurrence, so IoU never exceeds 0.5. Tune `rho ∈ {0.6,
  0.7, 0.8, 0.9}`, `warping ∈ {true, false}`, and the length window factor.
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
  (20 000 points ⇒ ≈ 1 MB).
- Query: PAA of the prototype → `LB = RMS distance between PAA vectors`. For windows of length
  `L*` and Euclid-type distances this is a true lower bound of the RMS distance (Keogh et al.,
  2001), so scanning in LB order and stopping when `LB ≥ best` is exact. For other lengths
  (resampling) and DTW it is used as a filter: exact distances on the 32 best candidates. Refine the
  winner with a step-1 search in `±s/2`. Sketches are z-normalised only for z-distances.
- Expected cost per evaluation: ~3·n/s·16 flops + few exact distances ⇒ ~10⁵ flops for n = 20 000.

### Objectives (all minimised)

| | Value | Normalised |
|---|---|---|
| f1 | coverage `Σ lᵢ / n` (maximise) | `1 − coverage / coverage_max`, `coverage_max = max_occ·max_len/n` capped at 1 |
| f2 | extent `E / δ(L*)` (minimise) | clamp to `[0, 1.5]` then `/1.5` |
| f3 | margin `μ` (maximise) | `(1 − clamp(μ, −1, 1)) / 2` |

Why it fixes the symptoms: adding a weak occurrence raises `E` (f2 worse) and lowers `μ` (f3 worse);
missing a true occurrence leaves it as nearest outsider (low `d_out`, f3 worse); a tiling of the
series has `E/δ ≈ 1` and `μ ≤ 0`; length no longer has its own objective, so occurrences grow only
while they stay closed. The two-occurrence bias disappears with leave-one-out.

### Quality score (ranking, labelling weights)

`Q = coverage · max(μ, 0)` — zero for sets that are not closed. Configurable `rank_by: quality |
ideal` (ideal = current behaviour) for the ablation.

### Feasibility and complexity

- `min_std_rel` (0.05): a window is infeasible if its std < 0.05·std(series) (replaces the absolute
  1e-6 as default; `min_std` kept as absolute floor).
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
- `top_k: auto`: candidates with `μ > 0` only.

## R5. Ablation protocol

- Variants (benchmark methods with labels): `v1` (feature 001 defaults with the R1 stretch fix),
  `full`, and `full − X` for X ∈ {loo, margin, coverage, scale, refine, collective, guarded_add,
  mating, split, seed_nn, archive, immigrants, quality_rank, dedupe_v2, labelling}.
- **Sample size** — Decision: 30 series per dataset and split, 3 seeds averaged per series before
  testing. Rationale: with n = 10 the smallest two-sided exact Wilcoxon p is 2/2¹⁰ ≈ 0.002, above
  the first Holm threshold 0.05/15 ≈ 0.0033 only when all 10 series agree, so the strict rule would
  delete almost everything; n = 30 detects moderate effects. Alternatives: 10 series (too weak);
  (series, seed) pairs as samples (not independent, inflates significance).
- **Splits** — Decision: the ablation runs on validation and test; decisions use validation only;
  test ΔF1 is reported. Datasets are identified by `(name, split)`; the synthetic validation split
  is the same generator with `seed + 10⁶`. Cost: 17 variants × 6 datasets × 2 splits × 30 series ×
  3 seeds = 18 360 runs × ~2 s ≈ 10 h sequential (overnight; `jobs` shortens it on many cores).
  Baselines are not part of the ablation config (they do not depend on components).
- **Test and correction** — Decision: paired Wilcoxon signed-rank (`zero_method="zsplit"`, two-sided)
  of per-series F1, `full` vs `full − X`; Holm across the 15 components within each
  `(dataset, split)`. Alternative rejected: Holm across components × datasets (90 tests; threshold
  0.05/90 unreachable even at n = 30 for small effects).
- **Decision rule** (validation only) — keep X iff removing it lowers F1 with Holm p < 0.05 on ≥ 1
  dataset and raises it with Holm p < 0.05 on none; every other component (including no significant
  effect) is deleted with its switch, config leaf and tests. Alternative rejected: keep neutral
  components that cost < 20 % time (dead weight under Principle III).
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

## R8. Ablation decisions

Filled after the study: one row per component (validation ΔF1 per dataset, Holm p, keep / delete),
plus the v1 decision.

## R6. Dependencies

- Rust: none new (PAA, DP, 2-medoids, bootstrap are a few lines each).
- Python bench extra: `scipy` (already transitive via `stumpy`; declared explicitly).
