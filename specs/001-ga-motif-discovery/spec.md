# Feature Specification: Evolutionary discovery of variable-length motifs

**Feature Branch**: `001-ga-motif-discovery`

**Created**: 2026-10-03

**Status**: Draft

**Input**: User description (translated): "Library (Rust core, Python API, CLI) that discovers, in
an unsupervised way, motifs in univariate time series with a multi-objective genetic algorithm that
evolves sets of variable-length candidate windows. CSV loading, preprocessing, YAML configuration,
decoupled logs, parallelism and visualisation of the series, the motifs and their evolution."

## Glossary

- **Series**: ordered sequence of real values, univariate.
- **Segment**: contiguous stretch of the series defined by (start, length).
- **Motif**: set of ≥ 2 non-overlapping segments, possibly of different lengths, that share a
  shape. Its **prototype** is the mean shape of its segments brought to a common length.
- **Support**: number of segments (occurrences) of a motif.
- **Cohesion**: mean dissimilarity between each occurrence and the prototype, normalised by length.
- **Stretch**: ratio between the longest and the shortest occurrence of a motif.

## Clarifications

### Session 2026-10-03

- Q: When a result is saved to disk, is the original series saved too? → A: Yes, the original series is always saved with the result, whether it came from a file or from memory.
- Q: When does a found occurrence recover a planted one? → A: When their IoU (intersection ÷ union of the two segments) is ≥ 0.5.
- Q: Must the visualisation HTML open without an internet connection? → A: Yes; the plotting library is embedded by default (works offline) and can optionally be loaded from a CDN.
- Q: How are the K returned motifs ordered? → A: First by Pareto front (rank) and, within each front, by closeness to the ideal point.
- Q: On what hardware are the time and speed-up targets measured? → A: On the developer's machine as reference (Apple M4, 10 cores 4P+6E, 16 GB, macOS 26), plus a relative regression check (> 10 %) against a saved baseline.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Discover motifs from a notebook (Priority: P1)

A researcher has a series (CSV file or in-memory array) and wants to get, without labels, the most
relevant recurring patterns even when each repetition lasts a different time, with a single call
and sensible default parameters.

**Why this priority**: it is the core value of the library; without it nothing else matters.

**Independent Test**: on a synthetic series with planted motifs, one call with a fixed seed
returns K distinct motifs with their occurrences, prototype and the values of the three
objectives, and the planted occurrences are recovered.

**Acceptance Scenarios**:

1. **Given** a series with 3 planted motifs of 8 occurrences each, with durations varying up to
   ±30 % and noise, **When** discovery runs with `min_len`/`max_len` that contain them, **Then**
   ≥ 90 % of the planted occurrences are recovered among the K returned motifs (IoU ≥ 0.5).
2. **Given** the same series, configuration and seed, **When** it runs twice with a different
   number of threads, **Then** the results are identical.
3. **Given** a CSV with several columns, **When** the column is given by name or index, **Then**
   that column is analysed; if it does not exist or is not numeric, an error lists the available
   columns.
4. **Given** a result, **When** it is queried, **Then** it offers the K distinct motifs and also
   the full Pareto front.

---

### User Story 2 - Preprocess the series declaratively (Priority: P2)

The user declares an ordered list of steps (handle gaps, remove trend, smooth, normalise, reduce
resolution) and motif positions are always reported in coordinates of the original series.

**Why this priority**: real series have gaps, trend and large lengths; without this, results are
of little use or slow.

**Independent Test**: a series with NaN and trend, preprocessed with interpolation + detrend +
×4 reduction, yields motifs whose positions point to the correct stretches of the original series.

**Acceptance Scenarios**:

1. **Given** a series with missing values, **When** interpolate or drop is chosen, **Then** no
   missing values remain and the reported indices refer to the original series.
2. **Given** resolution reduction by factor f, **When** a motif is discovered, **Then** its
   occurrences are reported in original indices (scaled by f).
3. **Given** an invalid step (e.g. smoothing window longer than the series), **Then** it is
   rejected before running with a clear message.

---

### User Story 3 - Run experiments from the command line with YAML (Priority: P2)

The researcher launches reproducible batch experiments from a terminal with a YAML file and
overrides individual parameters with arguments.

**Why this priority**: needed for parameter sweeps and running on servers.

**Independent Test**: `ditscover run cfg.yaml --seed 7 --out res/` writes the result, the history
and the effective configuration to `res/`; repeating the command produces an identical result.

**Acceptance Scenarios**:

1. **Given** a valid YAML, **When** `run` executes, **Then** results, history and effective
   configuration (with seed and version) are written.
2. **Given** a YAML with an unknown key or an out-of-range value, **Then** the run does not start
   and the key and reason are reported.
3. **Given** a parameter in the YAML and the same one as an argument, **Then** the argument wins.

---

### User Story 4 - Visualise motifs and their evolution (Priority: P3)

The user interactively sees the series with each motif's occurrences coloured, the overlay of
occurrences with their prototype, the evolution of objectives/hypervolume per generation and the
Pareto front over time.

**Why this priority**: essential for interpretation, but does not block discovery.

**Independent Test**: from a saved result, the views are generated in a self-contained HTML that
opens in the browser without a server.

**Acceptance Scenarios**:

1. **Given** a result, **When** the motif view is requested, **Then** each motif has a colour and
   its occurrences are shown on the series and overlaid, rescaled to a common length.
2. **Given** a history, **When** the evolution view is requested, **Then** the hypervolume and the
   best/mean of each objective are shown per generation, and the front can be browsed by
   generation.
3. **Given** the CLI with `--plot`, **Then** the HTML is saved next to the results.

---

### User Story 5 - Monitor and control long runs (Priority: P3)

The user sees progress, can stop the run (Ctrl+C or time limit) and get the best result so far,
and logging never slows down the algorithm.

**Why this priority**: usability on long series; protects performance.

**Independent Test**: a run with a 5 s budget finishes in ≤ 6 s with stop reason "time" and a
valid result; interrupting from Python returns a result with reason "cancelled".

**Acceptance Scenarios**:

1. **Given** a stagnation criterion (N generations without hypervolume improvement > δ), **When**
   it is met, **Then** the run ends with reason "stagnation".
2. **Given** a user interruption, **Then** the best result up to the last complete generation is
   returned with reason "cancelled".
3. **Given** a slow or saturated log sink, **Then** the algorithm does not stop; dropped events are
   counted and their number is reported.

### Edge Cases

- Series shorter than `2 × min_len`: error before running.
- Flat stretches (variance ~0): cannot form occurrences on their own (avoids trivial motifs).
- Series without structure (white noise): motifs with poor cohesion are returned, never a failure;
  cohesion lets the user discard them.
- `min_len == max_len`: behaves as a fixed-length search.
- Fewer distinct motifs than K: those that exist are returned.
- Column with all values missing, empty file or wrong delimiter: explicit error.
- Interruption before the first generation completes: empty result with reason "cancelled".

## Requirements *(mandatory)*

### Functional Requirements

**Data and preprocessing**

- **FR-001**: The system MUST load series from CSV/TSV (configurable delimiter, with or without
  header, column by name or index) and from in-memory arrays.
- **FR-002**: The system MUST offer these preprocessing steps: missing values (interpolate,
  forward-fill, drop), trend removal (linear, differencing), smoothing (moving average), global
  normalisation (z-score, min-max) and resolution reduction (block mean), applied in the declared
  order.
- **FR-003**: All reported positions MUST be indices of the original series.
- **FR-004**: Loading MUST be extensible to new formats (ARFF, etc.) without changing the rest.

**Individual representation**

- **FR-005**: Each individual MUST be a candidate motif: ordered list of k segments
  (start, length), with `min_occ ≤ k ≤ max_occ` (default 2 and 20).
- **FR-006**: Every individual MUST always satisfy: lengths in `[min_len, max_len]`, segments
  inside the series and non-overlapping, stretch ≤ `max_stretch` (default 1.5) and no segment with
  standard deviation < `min_std`. Genetic operators MUST repair any violation.
- **FR-007**: The prototype MUST be obtained by resampling each occurrence to the motif's mean
  length and averaging point by point.

**Similarity and objectives**

- **FR-008**: The dissimilarity MUST be selectable: z-normalised Euclidean (default), raw Euclidean
  and DTW with configurable band, always after resampling to a common length.
- **FR-009**: The algorithm MUST optimise three objectives simultaneously: maximise support,
  minimise cohesion and maximise normalised mean length, without weights, using Pareto dominance
  with elitism and diversity preservation.
- **FR-010**: The population MUST keep distinct motifs: individuals whose occurrences overlap more
  than `niche_jaccard` with a better or equal one are relegated in selection.

**Evolution**

- **FR-011**: The initial population MUST mix random individuals and seeded individuals (random
  seed + its best matches), in a configurable proportion.
- **FR-012**: The system MUST offer donation crossover (a child inherits one parent's occurrences
  and accepts those of the other parent that fit its prototype) and shift, stretch, add and remove
  mutations, with configurable probabilities.
- **FR-013**: Individual evaluation MUST run in parallel.
- **FR-014**: Combinable stop criteria: maximum generations, time budget, hypervolume stagnation
  and user cancellation; the result states which one fired.

**Results**

- **FR-015**: The result MUST contain the K distinct motifs (occurrence overlap ≤
  `dedupe_jaccard` between them), ordered first by Pareto front (rank) and, within each front, by
  closeness to the ideal point in the normalised objective space, and the full Pareto front.
- **FR-016**: Each motif MUST expose occurrences, prototype, support, cohesion, mean length and
  stretch.
- **FR-017**: Results MUST be savable to and loadable from disk together with the effective
  configuration, the seed, the version and the full original series (always, whether it came from
  a file or from memory), so a loaded result can be visualised without access to the source data.

**Configuration, interfaces and logging**

- **FR-018**: All configuration MUST be accepted via YAML, CLI arguments and API parameters, with
  the same names, precedence defaults < YAML < arguments and full validation before running.
- **FR-019**: The CLI MUST offer `run` (discover and save) and `plot` (generate HTML from a saved
  result).
- **FR-020**: Execution MUST be deterministic given the seed, regardless of the number of threads.
- **FR-021**: The system MUST record per generation: time, hypervolume, front size, best and mean
  of each objective and, every `snapshot_every` generations, the full front.
- **FR-022**: Logging MUST be decoupled from the algorithm and never block it; events that cannot
  be logged are dropped and counted.
- **FR-023**: Interactive visualisations: series + occurrences per motif, overlay of occurrences
  with prototype, evolution of objectives/hypervolume and front per generation. The generated HTML
  MUST be self-contained and work offline by default (plotting library embedded); loading it from
  a CDN is an explicit option.

### Key Entities

- **Series**: values + mapping of each index to the original series.
- **Segment**: (start, length) in indices of the preprocessed series.
- **Individual/Motif**: list of segments + objectives (support, cohesion, length) + rank and
  crowding distance in the population.
- **Config**: data, preprocessing, motif constraints, distance, GA parameters, stopping, output,
  logging, seed.
- **RunResult**: K motifs, front, history, stop reason, effective configuration, version,
  original series.
- **GenerationRecord**: statistics of one generation (+ optional front).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On synthetic series of 10,000 points with 3 planted motifs (8 occurrences, duration
  ±30 %, Gaussian noise σ = 0.1 of the amplitude), the median over 10 seeds recovers ≥ 90 % of the
  planted occurrences (IoU ≥ 0.5 between found and planted occurrence) with precision ≥ 80 % in
  the top 5 motifs.
- **SC-002**: That same run (population 200, 300 generations) finishes in < 30 s on the reference
  machine (Apple M4, 10 cores 4P+6E, 16 GB, macOS 26, `--release` build).
- **SC-003**: 100 % of repeated runs with the same seed produce identical results with 1 and with
  N threads.
- **SC-004**: Enabling full logging increases total time by < 5 %.
- **SC-005**: A new user gets and visualises motifs from a CSV in ≤ 3 lines of code or a single
  command.
- **SC-006**: Thread scaling reaches ≥ 5× speed-up with 8 threads versus 1 on the reference
  machine.
- **SC-007**: A time or cancellation stop returns a result ≤ 1 s after the signal.
- **SC-008**: On any machine, a reference benchmark does not get > 10 % worse than the baseline
  saved on that same machine.

## Assumptions

- v1 supports only univariate series and only CSV/TSV; ARFF and other formats are left for later
  features thanks to FR-004.
- Users: researchers and data scientists working in notebooks or the terminal.
- The series fits in memory (up to ~10⁶ points after preprocessing).
- `min_len`/`max_len` are set by the user (no automatic estimation in v1).
- Defaults: population 200, 300 generations, K = 5, `max_stretch` 1.5, `niche_jaccard` 0.5,
  `dedupe_jaccard` 0.3, z-normalised Euclidean distance.
- Visualisation produces self-contained HTML (offline by default); there is no web interface or
  server.
