# Feature Specification: Motif quality — fitness v2, smarter search, conflict-free labelling and ablation

**Feature Branch**: `003-motif-quality`

**Created**: 2026-10-13

**Status**: Draft

**Input**: User description (translated): "Using Spec Kit, create a feature with everything from the
review (benchmark fixes, fitness, operators, selection, labelling, ablation) to iterate
intelligently, fix the problem, improve the genetic algorithm and get better results on the
labelled dataset."

## Context: findings of the 2026-10-13 review (`results/bench-001`)

| Dataset | ditscover F1 | STOMP F1 | LoCoMotif F1 |
|---|---|---|---|
| synthetic variable-length | 0.22 | 0.41 | 0.00 |
| ECG5000 | 0.38 | 0.54 | 0.44 |
| Symbols | 0.53 | 0.54 | 0.78 |

- ditscover loses on **precision**: 46 occurrences per synthetic series vs 18 true (TP 419, FP 2 344).
  Support has no notion of a radius, so sets fill up with weak occurrences.
- Cohesion compares each occurrence with a prototype that includes itself (biased low for small
  sets, tolerant to outliers); the length objective pushes every occurrence to `max_len`; the top-K
  ranking picked a tiling of the whole series (cohesion 0.96) as motif #1; duplicates of the same
  shape survive deduplication; runs stop by stagnation after < 1 s.
- The benchmark itself is distorted: STOMP always returns one set, LoCoMotif scores TP = 0 on
  synthetic data, and ditscover runs with a stretch limit (1.5) below the stretch of the ground truth
  (1.86).

## Clarifications

### Session 2026-10-13

- Q: Scope? → A: everything in the review, organised so each change can be measured on its own.
- Q: How are changes accepted? → A: by an ablation study on the labelled benchmark, tuned on a
  validation split and reported once on the test split; components that do not help are removed.
- Q: When removing a component has no significant effect on validation F1, is it kept or
  deleted? → A: Deleted. A component is kept only if removing it significantly lowers F1
  (Holm-adjusted p < 0.05) on ≥ 1 dataset and significantly raises it on none.
- Q: Does the per-component ablation run on the test split too? → A: Yes, on both splits; keep/delete
  decisions use validation only, test ΔF1 is reported for transparency.
- Q: Is the previous fitness (v1) deleted if v2 wins? → A: Yes; v1 objectives, `rank_by: ideal` and
  the Jaccard dedupe are deleted once v2 significantly wins; recorded v1 results remain the baseline.
- Q: Under what settings must the 10 s / 20 000-point target hold? → A: Library defaults (including
  the minimum generations), single seed, all cores; median run time.
- Q: How many validation series per dataset, and over which tests is Holm applied? → A: 30 series
  per dataset (seeds averaged per series); Holm across the components within each dataset.

## Glossary

- **Member distance (leave-one-out)**: distance of an occurrence to the prototype of the *other*
  occurrences of its set.
- **Extent**: the largest (or a high quantile of the) member distances of a set.
- **Nearest outsider**: the window of the series most similar to the prototype that is not an
  occurrence of the set and does not overlap one.
- **Margin**: nearest-outsider distance minus extent; positive means every occurrence is closer to
  the pattern than anything else in the series (a "closed" set).
- **Coverage**: fraction of the series covered by the occurrences of a set.
- **Random-pair scale**: typical distance between two random windows of the same length in the
  series; used to make distances comparable across lengths and series.
- **Labelling**: assignment of each time point to at most one motif (or none).
- **Ablation**: measuring the effect of each component by switching it on or off.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Trustworthy comparison and error analysis (Priority: P1)

The researcher needs a benchmark whose numbers can be trusted before changing the algorithm:
every method correctly configured and tuned the same way, the discovered sets stored, errors
visible per series, and differences backed by a statistical test.

**Why this priority**: improvements cannot be measured on a distorted baseline.

**Independent Test**: re-running the benchmark gives Matrix Profile more than one set when more
exist, LoCoMotif non-zero scores on the synthetic suite, stored sets for every run, an error view
per series and paired test results; this becomes the recorded baseline.

**Acceptance Scenarios**:

1. **Given** `k` requested sets, **When** Matrix Profile runs, **Then** it may return up to `k`
   sets instead of being limited by a default cut-off.
2. **Given** the synthetic suite, **When** LoCoMotif runs with tuned parameters, **Then** its
   occurrences are in the same coordinates and length range as the ground truth (verified on
   stored sets).
3. **Given** a dataset with ground truth, **Then** every method receives the same length window
   and stretch range derived from it, and ditscover's stretch limit covers the ground-truth
   stretch.
4. **Given** a parameter grid per method, **When** the benchmark runs, **Then** parameters are
   chosen on a validation split and the test split is evaluated once with the chosen values.
5. **Given** finished runs, **Then** the discovered sets are stored per run and the report shows,
   per series, ground truth vs. found occurrences marked as hit, false alarm or miss.
6. **Given** two methods on the same series, **Then** the summary reports a paired significance
   test of their F1 per dataset.

---

### User Story 2 - A fitness that rewards real, closed motif sets (Priority: P1)

The discovery result must prefer sets whose occurrences are similar to each other and clearly
different from the rest of the series, without being pulled towards "as many occurrences as
possible" or "as long as possible".

**Why this priority**: it addresses the main cause of the poor precision.

**Independent Test**: on hand-built series, a true set beats the same set plus one junk occurrence,
beats the same set minus one true occurrence, and beats a tiling of the whole series; on the
synthetic suite precision rises well above the current 0.15.

**Acceptance Scenarios**:

1. **Given** a true set and the same set plus a weak occurrence, **Then** the extended set is
   worse in at least one objective and better in none.
2. **Given** a true set missing one of its occurrences, **Then** its margin is lower than that of
   the complete set.
3. **Given** a set of two occurrences, **Then** its member distances are not artificially halved.
4. **Given** flat or noise-only windows, **Then** they cannot form motifs.
5. **Given** a DTW-based run, **Then** occurrences are compared after per-window normalisation and
   against a representative member, so level differences do not break matches.
6. **Given** the previous fitness, **Then** it remains selectable so both can be compared, and is
   deleted once the ablation shows the new one significantly better.

---

### User Story 3 - A search that aligns boundaries and starts from good candidates (Priority: P1)

The algorithm must place occurrence boundaries precisely, move whole motifs coherently, avoid
bloating sets, start from strong candidates and use the available time instead of stopping early.

**Why this priority**: matching requires > 50 % overlap with the true occurrence; misaligned
boundaries turn near-hits into misses.

**Independent Test**: from a set whose occurrences are shifted by 15 % of their length, the search
recovers the planted boundaries; initial populations already score at least as well as Matrix
Profile seeds; runs do not stop before the minimum number of generations.

**Acceptance Scenarios**:

1. **Given** occurrences shifted or resized by up to 15 %, **When** boundary refinement runs,
   **Then** each recovers its planted boundaries within 2 points on noise-free data.
2. **Given** a motif whose whole phase is wrong, **Then** a single collective move can shift or
   rescale all its occurrences at once.
3. **Given** a set, **When** occurrences are added by mutation or crossover, **Then** an addition is
   accepted only if it does not worsen the set's extent.
4. **Given** a set mixing two patterns, **Then** a split can produce a child with one pattern.
5. **Given** the start of a run, **Then** part of the population is seeded from the best
   nearest-neighbour pairs of the series, each grown until the first large jump in distance.
6. **Given** stagnation, **Then** the run continues until a minimum number of generations and
   injects new candidates from uncovered regions before stopping.
7. **Given** distinct good motifs found during the run, **Then** none is lost because the
   population moved on (they are kept in an archive).

---

### User Story 4 - Distinct motifs and a conflict-free labelling (Priority: P2)

The final output is K distinct motifs and a labelling of the series where every time point belongs
to at most one motif.

**Why this priority**: overlapping occurrences of two motifs are guaranteed false alarms, and users
need a clean segmentation.

**Independent Test**: on the electric-production series the top motifs are no longer the same
shape; on any result no two reported occurrences overlap; the labelling vector is available and
drawn in the report.

**Acceptance Scenarios**:

1. **Given** final candidates, **Then** they are ranked by a quality score (margin and coverage),
   never by raw support.
2. **Given** a candidate whose occurrences are mostly contained in a better one, or whose shape
   matches a better one, **Then** it is discarded as a duplicate.
3. **Given** chosen motifs with overlapping occurrences, **Then** conflicts are resolved by keeping
   the set of occurrences with the highest total quality; motifs left with too few occurrences are
   dropped.
4. **Given** a result, **Then** it includes a label per time point (motif number or none) and the
   report shows it as a band under the series.
5. **Given** `top_k: auto`, **Then** only motifs with a positive margin are returned.

---

### User Story 5 - Ablation study and new defaults (Priority: P2)

The researcher runs one benchmark that compares the previous algorithm, the full new algorithm and
the full algorithm minus each component, and adopts as defaults only what helps.

**Independent Test**: one command produces the ablation table (ΔF1 per component and dataset with
paired tests) and the recommended defaults.

**Acceptance Scenarios**:

1. **Given** the ablation configuration, **Then** each variant runs on the same series and seeds.
2. **Given** results, **Then** the report shows, per component, the F1 change when it is removed,
   with significance, on validation and test (decisions are taken on validation only).
3. **Given** a component whose removal does not significantly lower validation F1 on any dataset
   (Holm-adjusted p < 0.05), or significantly raises it on one, **Then** it is removed from the code
   (not left as an option) and the decision is recorded.

### Edge Cases

- Series too short for any outsider window (all windows overlap the set): margin is undefined and
  treated as 0.
- Periodic series where every window resembles every other: margins are small; `top_k: auto` may
  return no motif, which is a valid answer.
- All occurrences flat after the relative variance filter: no feasible motif → clear error.
- DTW with sets of two occurrences: the representative member is either of them; distances are
  symmetric.
- Labelling removes so many occurrences that fewer than K motifs remain: return the remaining ones.
- Tuning grid larger than the time budget: the benchmark refuses before running and reports the
  estimated number of runs.

## Requirements *(mandatory)*

### Functional Requirements

**Measurement (US1)**

- **FR-001**: Matrix Profile MUST be able to return up to `k` sets (no default distance cut-off on
  the number of motifs).
- **FR-002**: The LoCoMotif adapter MUST be verified against stored outputs (coordinates, lengths)
  and corrected if needed.
- **FR-003**: For labelled datasets, every method MUST receive the same length window and stretch
  range derived from the ground truth; ditscover's stretch limit MUST cover it.
- **FR-004**: Benchmarks MUST support per-method parameter grids tuned on a validation split, then a
  single evaluation on the test split; synthetic suites MUST provide a separate validation suite.
- **FR-005**: Discovered sets MUST be stored for every run.
- **FR-006**: The benchmark report MUST include an error view per series (hits, false alarms,
  misses per method).
- **FR-007**: Summaries MUST include paired significance tests (per dataset, between every pair of
  methods) on F1.

**Fitness v2 (US2)**

- **FR-008**: Member distances MUST be leave-one-out; the set's extent MUST be a configurable high
  quantile of them (default 0.9).
- **FR-009**: The fitness MUST include the margin to the nearest outsider window (searched over the
  allowed lengths, excluding windows overlapping the set).
- **FR-010**: The fitness MUST use coverage instead of separate support and length objectives.
- **FR-011**: Distances in objectives MUST be divided by the random-pair scale for that length.
- **FR-012**: Windows with variance below a fraction of the series variance MUST be infeasible; a
  complexity-invariant correction MUST be available.
- **FR-013**: A per-window normalised DTW MUST be available, with a representative member
  (medoid) as prototype for DTW distances.
- **FR-014**: The previous fitness MUST remain selectable until the ablation decides. If the full
  new algorithm significantly beats it on validation, the previous fitness, its ranking and its
  deduplication MUST be deleted from the code; its recorded benchmark results remain the baseline.

**Search (US3)**

- **FR-015**: A boundary refinement local search MUST adjust start and length of each occurrence to
  minimise its member distance, applied with a configurable probability.
- **FR-016**: Collective mutations MUST shift all occurrences together and rescale all lengths
  together.
- **FR-017**: Additions by mutation or crossover MUST be accepted only if the extent does not
  increase; removals MUST target the worst member when that increases the margin.
- **FR-018**: Crossover MUST pair parents with similar prototype or overlapping coverage; otherwise
  only mutation applies.
- **FR-019**: A split mutation MUST divide a set into two groups when its members form two clusters.
- **FR-020**: Initialisation MUST seed a configurable share of the population from the best
  nearest-neighbour pairs, grown until the largest relative jump in sorted distances.
- **FR-021**: An archive of distinct non-dominated motifs MUST be kept during the run; stagnation
  MUST be measured on it; the final selection MUST draw from it.
- **FR-022**: The run MUST not stop by stagnation before a minimum number of generations and MUST
  inject candidates from uncovered regions when stagnating.

**Selection and labelling (US4)**

- **FR-023**: The final ranking MUST use a quality score combining margin and coverage.
- **FR-024**: Deduplication MUST use containment (overlap relative to the smaller set) and
  prototype shape similarity tolerant to small lags.
- **FR-025**: Final occurrences MUST be conflict-free across motifs, resolved by maximum total
  quality; the result MUST include a label per time point.
- **FR-026**: `top_k` MUST accept `auto` (motifs with positive margin).

**Ablation (US5)**

- **FR-027**: Every new default-on component MUST be switchable for the ablation; the benchmark MUST run the
  previous algorithm, the full new one and full-minus-each-component on the same series and seeds.
- **FR-028**: The report MUST show the F1 change per removed component with significance, on
  validation and test; keep/delete decisions MUST use validation results only, test results are
  reported but never change a decision. The ablation MUST use 30 validation series per dataset, with
  seeds averaged per series before a paired Wilcoxon test, and Holm correction applied across the
  components within each dataset.
- **FR-029**: FR-008–FR-024 describe candidate components and stay binding only while they survive
  the ablation; conflict-free labelling (FR-025) is an output guarantee (SC-004), measured in the
  ablation but never deleted. After the ablation, a component MUST be kept only if removing it significantly lowers
  validation F1 (Holm-adjusted p < 0.05) on ≥ 1 dataset and significantly raises it on none; every
  other component (including those with no significant effect) MUST be deleted and defaults
  updated; decisions MUST be recorded in the feature's research notes.

**Ablation outcome** (research R8): binding — FR-008, FR-013, FR-015, FR-017 (guarded addition,
donation and removal), FR-020, FR-022 (minimum generations only), FR-023, FR-024, FR-025, FR-026;
the margin to the nearest outsider and the random-pair scale are still computed for ranking,
labelling and the guarded operators. Retired — FR-009/FR-010/FR-011 as objectives (objectives are
support, extent and cohesion), FR-012 relative variance floor (the absolute `min_std` remains),
FR-016, FR-018, FR-019, FR-021, FR-022 injection; FR-014 resolved by deleting the previous fitness.

### Key Entities

- **Candidate index**: compact per-length sketches of all windows, used to find nearest outsiders,
  additions and seeds quickly.
- **Fitness v2 values**: coverage, normalised extent, normalised margin, quality score.
- **Labelling**: one motif number (or none) per original time point.
- **Run set record**: discovered sets of one benchmark run.
- **Ablation variant**: a labelled ditscover configuration with one component switched.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Synthetic suite (test split): median F1 of ditscover ≥ 0.70 and precision ≥ 0.70
  (currently 0.22 and 0.15).
- **SC-002**: ditscover's F1 is not significantly lower than the best corrected baseline on any of
  the labelled datasets, and significantly higher on the synthetic variable-length suite.
- **SC-003**: Symbols F1 ≥ 0.75 and ECG5000 F1 ≥ the corrected Matrix Profile.
- **SC-004**: No reported result contains two overlapping occurrences (0 conflicts in 100 % of runs).
- **SC-005**: Median run time stays below LoCoMotif's on every dataset, and a run with the library
  defaults (including the minimum number of generations), a single seed and all cores takes a
  median < 10 s on a 20 000-point series.
- **SC-006**: Every component kept as default significantly improves validation F1 (Holm-adjusted
  p < 0.05) on at least one dataset without a significant loss on another; 100 % of other components
  are deleted from the code (conflict-free labelling excepted, FR-025).
- **SC-007**: Repeated runs with the same seed remain identical (as in features 001-002).

## Assumptions

- The five univariate TSMD-Bench datasets (ECG5000, Fungi, Plane, Mallat, Symbols) and the synthetic
  suite are the evaluation set; each has a validation and a test split.
- Methods keep receiving the number of ground-truth sets (`k = oracle`) in benchmarks.
- Tuning grids stay small (≤ 12 combinations per method); the ablation (30 validation series per
  dataset and split × 3 seeds, both splits) is expected to fit in an overnight run (~11 h
  sequential).
- An MDL-based single objective is out of scope; it can be a later feature if the margin-based
  fitness falls short.
- Targets in SC-001..SC-003 are goals for the study; whatever the outcome, the benchmark reports it
  honestly.
