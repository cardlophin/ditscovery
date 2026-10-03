# Research: Evolutionary discovery of variable-length motifs

Notation: preprocessed series `x[0..n)`, motif `M = {(s_i, l_i)}_{i=1..k}`, `ρ = max_stretch`,
`L* = round(mean(l_i))`.

## R1. Genotype: list of segments (local Pittsburgh)

- **Decision**: individual = motif = `Vec<Segment{start: u32, len: u32}>` sorted by `start`,
  `min_occ ≤ k ≤ max_occ`. Variable-length genome; every occurrence has its own length.
- **Rationale**: elastic occurrences without a privileged seed; operators act directly on what the
  user interprets (occurrences).
- **Alternatives**: (start, L, r) with fixed length (rigid); seed + ε with uniform scaling (derived
  occurrences, less control); free-form prototype (huge space); memetic with subsequence-DTW
  (expensive, possible future feature).

## R2. Invariants and repair

Every operator ends in `repair`, so infeasible individuals never exist (no constraint handling in
selection → less code):

1. `len ← clamp(len, min_len, max_len)`, `start ← clamp(start, 0, n − len)`.
2. Stretch: with `m = median(l_i)`, `len ← clamp(len, m/√ρ, m·√ρ)` keeping the centre
   ⇒ `max/min ≤ ρ`.
3. Drop segments with `std < min_std` (prefix sums of `x` and `x²`, O(1) per segment).
4. Overlaps (with gap `min_gap`): sorted sweep; on conflict keep the segment closest to the current
   prototype.
5. `k > max_occ` ⇒ remove the worst. `k < min_occ` ⇒ *add* (best match, no quality limit) up to
   `min_occ` (max. 3 attempts); if it still fails, the operator returns its parent unchanged
   (implementation: simpler than re-seeding and equally feasible).

## R3. Phenotype, prototype and distance

- Each occurrence is resampled by linear interpolation to `L*` (uniform scaling) and, for
  `zeuclid` only, z-normalised (`std < ε` ⇒ zero vector). `euclid` and `dtw` compare raw values.
- Prototype `p = mean_i(v_i)` (pointwise).
- `enum Distance { ZEuclid, Euclid, Dtw { band: f64 } }`; `d(v, p)` normalised by `√L*`.
  DTW with Sakoe-Chiba band `w = ceil(band·L*)`, O(L*·w), one memory row.
- **Rationale**: centroid O(k·L*) vs O(k²·L*) for mean pairwise distance; `√L*` normalisation
  makes motifs of different lengths comparable.
- **Alternatives**: medoid (more robust, O(k²)); DBA for DTW (more expensive; future improvement
  if DTW becomes the main mode).

## R4. Objectives (NSGA-II, all minimised internally)

| Objective | Raw value | Normalised to [0,1] |
|---|---|---|
| f1 Support | `k` (maximise) | `(max_occ − k)/(max_occ − min_occ)` |
| f2 Cohesion | `mean_i d(v_i, p)` (minimise) | `f2 / c_ref`, `c_ref` = max of the initial population (fixed) |
| f3 Length | `mean(l_i)` (maximise) | `(max_len − mean l)/(max_len − min_len)` (0 if equal) |

- Real conflicts: more or longer occurrences ⇒ worse cohesion. No weights.
- For ZEuclid, `c ≤ 2`, which bounds the space.
- Dominance uses normalised values (equivalent); hypervolume uses normalised values clipped to
  `[0, 1.1]` with reference point `(1.1, 1.1, 1.1)`.

## R5. Diversity: clearing by occurrence overlap

- **Problem**: NSGA-II crowding acts in objective space; two different motifs with similar
  objectives compete and the population converges to a single pattern.
- **Decision**: after non-dominated sorting of `R = P ∪ Q`, walk `R` by (rank, −crowding); an
  individual whose temporal coverage has Jaccard > `niche_jaccard` with a previous "winner" moves
  to the last front. Jaccard by sorted interval intersection, O(kA + kB). Exact genotype
  duplicates are removed first.
- **Alternatives**: fitness sharing (needs σ_share, O(N²) shape distances per generation); islands
  (more code).

## R6. Initialisation

- `init_greedy` (default 0.5) seeded (FR-011 "seed + its best matches"): random seed segment
  `(s, L)`; strided scan of the whole series (stride `scan_step·len`) at lengths `L/√ρ, L, L·√ρ`;
  accept the closest non-overlapping windows until `k ~ U[min_occ, max_occ]`.
  *Changed during implementation*: the original `4·max_occ` random probes almost never hit the
  other occurrences of a motif in a 10⁴-point series (~0.5 % per probe), so clean motifs were
  never seeded.
- Rest random: `k ~ U[min_occ, max_occ]` random segments with common base length ± ρ.
- All go through `repair`.

## R7. Variation

- **Selection**: binary tournament by (rank, crowding).
- **Donation crossover** (`p_c` = 0.9): `A' = A ∪ {b ∈ B : d(b, p_A) ≤ max_i d(a_i, p_A)}` and the
  symmetric `B'`; `repair` using the receiver's prototype. Semantically meaningful (the child
  absorbs occurrences from the other parent that fit its shape) and never mixes incompatible motifs.
- **Mutations**, independent (default probabilities):
  - `shift` 0.3: one segment, `δ ~ round(N(0, σ_shift·len))`, `σ_shift = 0.1`.
  - `stretch` 0.2: one segment, `len·exp(N(0, σ_stretch))` centred, `σ_stretch = 0.1`.
  - `add` 0.2: best match of the prototype at the motif's mean length (strided scan + step-1
    refinement around the best hit), added only if its distance ≤ the current worst occurrence's
    (same rule as donation crossover). *Changed during implementation*: random probes always
    added the closest probe, even junk, which padded every motif to `max_occ`.
  - `remove` 0.1 (only if `k > min_occ`): remove the worst with prob. 0.5, otherwise a random one.

## R8. Survival, stopping and hypervolume

- Standard NSGA-II on `R` (2N): ranking (O(MN²), fine for N ≤ ~1000) + clearing + crowding.
- 3D HV by slicing: sort by f3, accumulate 2D HV by sweep. O(n² log n), n ≤ N.
- Stopping (OR): `max_generations`, `time_budget_s`, stagnation (`best_g − best_{g−window} <
  delta`, with `best_g` the highest HV seen up to generation g — clearing can remove front members,
  so the per-generation HV is not monotone and would trigger stagnation spuriously),
  cancellation (`AtomicBool` checked every generation). `StopReason` in the result.

## R9. Top-K result (spec clarification: rank, then ideal point)

- Sort the final population by (rank, ‖f_norm‖₂) — Pareto rank first, then distance to the ideal
  point `(0,0,0)` within each rank, ties by index. Greedy selection skipping motifs with Jaccard >
  `dedupe_jaccard` against those already chosen; up to K.
- The full first front is also returned.

## R10. Determinism with Rayon

- RNG `ChaCha8Rng` seeded with `splitmix64(seed ⊕ gen·C1 ⊕ idx·C2 ⊕ stream)`; child `i` of
  generation `g` uses its own RNG ⇒ independent of the scheduler.
- `par_iter().map().collect()` preserves order; reductions and sorts are sequential with index
  tie-break. Evaluating one individual is sequential internally.

## R11. Decoupled logging

- `crossbeam_channel::bounded(channel_capacity)`; the engine calls `try_send(Event)`; on failure it
  increments an `AtomicU64` of dropped events. Never blocks.
- Consumer thread: if `output.dir` is set, streams `history.jsonl` there (BufWriter); emits
  `tracing::info!` every `log_every` generations; accumulates the `GenerationRecord`s returned on
  `join`.
- **Alternatives**: `tracing-appender::non_blocking` alone (no recoverable structured history);
  writing from the engine (blocks).

## R12. Python, cancellation and CLI

- `crates/py` with PyO3 + `numpy` (zero-copy input via `PyReadonlyArray1<f64>`) and `pythonize`
  (Config/Result ↔ dict) ⇒ almost no conversion code.
- Cancellation (part of the US1 binding, not deferred — Constitution II): the engine runs in a
  thread; the Python thread releases the GIL and every 100 ms re-acquires it for
  `py.check_signals()`; on `KeyboardInterrupt` it sets the `AtomicBool` and waits for the partial
  result.
- **CLI in Python** (`ditscover run|plot`, wheel console script, `argparse`): one `pip install`
  gives API + CLI + plots, and `plot` reuses the same Plotly code. Config validation stays in
  Rust. *Rejected*: Rust binary with `clap` (cannot draw Plotly charts without duplicating the
  visualisation and is not shipped in the same wheel).
- Overrides by unique leaf name (`min_len=12`, `--set population=300`): the default Config is
  serialised to a tree, the leaf is located and replaced; unknown name ⇒ error.

## R13. CSV reading (deviation from the constitution's base dependencies)

- **Decision**: `csv` crate (+ `serde`) instead of `polars`.
- **Rationale**: only one numeric column is needed; `polars` multiplies compile time and wheel
  size (tens of MB) without adding value here. The builder API of the existing `CsvTSLoader` is
  kept (column by name/index, delimiter, header). Recorded in plan.md Complexity Tracking.
- **Alternatives**: `polars` (existing; heavy), `arrow-csv`.

## R14. YAML

- **Decision**: `serde_yaml_ng` 0.10 (maintained fork of the deprecated `serde_yaml`) with
  `#[serde(deny_unknown_fields)]` to reject unknown keys.
- **Alternatives**: `serde_norway` (another fork, smaller user base); `serde_yaml` (unmaintained).

## R15. Visualisation (spec clarification: offline by default)

- `plotly` in Python: `motifs` (series + coloured occurrences), `overlay` (aligned occurrences +
  prototype per motif, both computed by the core — see R17), `evolution` (HV and objectives per
  generation), `front` (3D scatter with a slider over snapshot generations). `Result.plot()`
  composes them into one self-contained HTML with `include_plotlyjs=True` (embedded, works
  offline, ~3.5 MB); `cdn=True` switches to `include_plotlyjs="cdn"`.

## R16. Validation with planted motifs (spec clarification: IoU)

- `synth::planted(n, lens, occ, stretch, noise, seed)` in the core: mean-reverting random-walk
  background (AR(1), φ = 0.98); each occurrence *replaces* the background with `level + shape`
  (damped sine, triangle, smooth step; amplitude 1) with length `L·U(1−s, 1+s)`; gaussian noise
  everywhere. Adding shapes on top of the background (first version) made the damped sine's
  planted set less cohesive (0.43) than background junk.
- Metric: a found occurrence recovers a planted one when IoU (intersection ÷ union of the two
  segments) ≥ 0.5; one-to-one greedy matching; recall and precision over the top-K.

## R17. Allocation-free hot loop (Constitution VI)

- **Decision**: `Scratch { a: Vec<f64>, b: Vec<f64>, row: Vec<f64> }` sized to `max_len`, created
  once per Rayon worker with `par_iter().map_init(Scratch::new, …)`. `resample`, `znorm`, `dtw` and
  `evaluate` write into caller-provided slices; the prototype accumulates in place.
- Determinism: scratch contents are always overwritten before being read, so which worker evaluates
  an individual cannot change the result (R10 still holds).
- Verified by `crates/core/tests/alloc.rs`: a counting global allocator asserts that 100 calls
  to `evaluate` (zeuclid, euclid, dtw) perform zero heap allocations.
- **Alternatives**: thread-local `RefCell` (hidden global state); allocating per call (violates VI).

## R18. Persisting results (spec clarification: always save the original series)

- **Decision**: a results directory holds `results.json` (RunResult without `history` and
  `series`), `history.jsonl`, `config.yaml` and `series.csv` (one column `value`, original series
  including NaN written as `NaN`). `load` reads `series.csv` back with the same `CsvLoader`.
- **Rationale**: JSON cannot encode NaN (the original series may have missing values); a
  one-column CSV reuses the existing loader and needs no new dependency.
- **Alternatives**: `.npy` (needs a crate on the Rust side); series inside `results.json` (NaN
  breaks round-trip, file bloats).

## R19. Two validation stages, one module

- `Config::validate()` checks everything that depends only on the config (ranges, `max_len ≥
  min_len`, step parameters ≥ 2, probabilities). Runs before loading data.
- `config::validate_step(i, &Step, n)` is called by `preprocess::apply` before each step `i`, with
  `n` the current length (e.g. `smooth`/`paa` window > current length ⇒ error with path `preprocess[2].smooth`), and
  `config::validate_len(n, &Config)` checks `n ≥ 2·min_len` after preprocessing.
- All checks live in `config.rs` and run before the engine starts (state `Prepared`), keeping
  Constitution VIII ("validation in one place, before running").

## R20. Benchmark regression gate (SC-002, SC-006, SC-008)

- Reference machine: Apple M4 (4P+6E), 16 GB, macOS 26, `--release`.
- Benches `ga/run_10k` (SC-002), `ga/threads_{1,8}` (SC-006), `ga/log_{on,off}` (SC-004),
  `distance/*` (hot loop).
- Baseline: `cargo bench -p ditscover-core --bench ga -- --save-baseline ref` on the reference
  machine; `cargo bench -p ditscover-core --bench ga -- --baseline ref` compares; `scripts/bench_check.py` reads
  criterion's `target/criterion/*/change/estimates.json` and exits 1 if any mean change > +10 %.
  Not in CI (shared runners are too noisy); run before merge, as the constitution requires.
- Risk: the M4 has 4 performance cores; ≥ 5× with 8 threads (SC-006) may not be reachable. If the
  bench shows it, revise SC-006 in the spec instead of over-optimising.
