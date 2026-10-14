---
description: "Task list: motif quality (fitness v2, search, labelling, ablation)"
---

# Tasks: Motif quality — fitness v2, smarter search, conflict-free labelling and ablation

**Input**: Design documents from `/specs/003-motif-quality/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: mandatory (Constitution IV). Each test block is written first and MUST fail.

## Format: `[ID] [P?] [Story] Description`

---

## Phase 1: Setup

- [X] T001 Declare `scipy` in the `bench` extra and add a `slow` pytest marker excluded by default (`addopts = "-m 'not network and not slow'"`) in `pyproject.toml`; add empty modules `index`, `fitness`, `archive`, `label` to `crates/core/src/lib.rs`; on `main` before any change, save the criterion baseline with `cargo bench -p ditscover-core --bench ga -- --save-baseline pre-003`
- [X] T002 Spike (not committed): run LoCoMotif on 3 synthetic series with stored outputs; compare found lengths / coordinates with GT; record the cause and the fix in `specs/003-motif-quality/research.md` (R1, "LoCoMotif diagnosis")

---

## Phase 2: Foundational

**⚠️ Blocks US2–US5**

### Tests (first)

- [X] T003 [P] `crates/core/tests/config.rs`: new leaves and defaults (data-model.md), validation errors with key path, flat overrides (`refine=0`, `top_k=auto`, `kind=zdtw`); golden test: `model: v1` + all components off reproduces the feature-001 result for a fixed seed bit for bit
- [X] T004 [P] `crates/core/tests/index.rs`: nearest outsider equals brute force for same-length Euclid-type queries (proptest); excluded windows never returned; deterministic build; memory bound at 20 000 points
- [X] T005 [P] `crates/core/src/fitness.rs` (`#[cfg(test)]`): `NullScale` deterministic, positive, interpolates between grid lengths

### Implementation

- [X] T006 New `fitness` section and `ga` / `stop` / `output` leaves with validation in `crates/core/src/config.rs`
- [X] T007 [P] `CandidateIndex` (PAA sketches, LB-ordered scan, refinement) in `crates/core/src/index.rs`
- [X] T008 [P] `NullScale` in `crates/core/src/fitness.rs`
- [X] T009 Build index and null scale once per run and expose them in `Ctx` (`crates/core/src/motif.rs`, `crates/core/src/engine.rs`)

**Checkpoint**: T003–T005 green; v1 golden test green

---

## Phase 3: User Story 1 - Trustworthy comparison and error analysis (P1) 🎯 MVP

**Goal**: a benchmark whose numbers can be trusted, recorded as the corrected baseline.

**Independent Test**: corrected baseline `results/bench-002` with stored sets, Errors tab and paired tests (quickstart §1).

### Tests (first)

- [X] T010 [P] [US1] `tests/python/test_methods.py`: STOMP returns > 1 set when several motifs exist; LoCoMotif hits a planted motif with matching coordinates (per T002); ditscover receives `max_stretch` covering the GT stretch
- [X] T011 [P] [US1] `tests/python/test_bench.py`: grid tuned on validation picks the best params of a fake method and evaluates test once; `max_runs` guard counts tuning + evaluation runs and refuses with the estimate; synthetic `split: validation` uses `seed + 1_000_000` and differs from test; the same dataset on two splits yields distinct run keys and `sets.jsonl` lines (resume-safe); paired Wilcoxon (`zero_method="zsplit"`) on seed-averaged per-series F1 with Holm across method pairs within each `(dataset, split)`, checked on a hand-made table
- [X] T012 [P] [US1] `tests/python/test_report.py`: Errors tab marks hits, false alarms and misses for a hand-made series
- [X] T013 [P] [US1] `crates/core/tests/bench.rs`: `tune`, `grid`, `max_runs`, `stats`, `save_sets`, `SyntheticSpec.split` parse and validate; a dataset listed on both splits is accepted, a duplicate `(name, split)` is rejected with its key path

### Implementation

- [X] T014 [US1] STOMP `cutoff=np.inf`, LoCoMotif fix from T002, ditscover stretch window in `python/ditscover/bench/methods.py`
- [X] T015 [P] [US1] Synthetic `split` (validation = `seed + 1_000_000`) and `max_stretch` from GT in `python/ditscover/bench/datasets.py`
- [X] T016 [US1] `tune`, `grid`, `max_runs`, `stats`, `save_sets`, `SyntheticSpec.split`, `(name, split)` dataset identity in run keys in `crates/core/src/bench.rs`
- [X] T017 [US1] Tuning loop, `sets.jsonl`, paired tests (seeds averaged per series, `scipy.stats.wilcoxon`, Holm within `(dataset, split)`) and `tuned` / `tests` (with `split`) in `summary.json` in `python/ditscover/bench/__init__.py`
- [X] T018 [US1] Errors view in `python/ditscover/plot.py` and Errors tab in `python/ditscover/report.py`
- [X] T019 [US1] Run the corrected baseline to `results/bench-002` (`specs/003-motif-quality/contracts/baseline.example.yaml`: the 002 datasets + Fungi, Plane, Mallat, tuned grids, ditscover v1 with the stretch fix; yfinance left out, cost-only); record per-dataset F1 / precision / recall in `specs/003-motif-quality/research.md` ("Corrected baseline")

**Checkpoint**: baseline recorded; quickstart §1

---

## Phase 4: User Story 2 - Fitness that rewards closed motif sets (P1)

**Goal**: fitness v2 (LOO extent, margin, coverage, random-pair scale) selectable next to v1.

**Independent Test**: hand-built series behave as in the acceptance scenarios; synthetic precision rises (quickstart §2).

### Tests (first)

- [X] T020 [P] [US2] `crates/core/tests/fitness.rs`: true set dominates set + junk occurrence; margin(true) > margin(true − one); tiling dominated; k = 2 member distances not halved; flat / noise windows infeasible with `min_std_rel`; normalised objectives in [0, 1]; margin = 0 when no outsider window exists
- [X] T021 [P] [US2] Proptest in `crates/core/tests/fitness.rs`: adding an occurrence farther than the extent never improves f2 or f3
- [X] T022 [P] [US2] `crates/core/src/distance.rs` tests: `zdtw` invariant to level and amplitude; CID factor ≥ 1 and = 1 for equal complexity
- [X] T023 [P] [US2] `crates/core/tests/fitness.rs` (with the other fitness tests): medoid prototype for DTW kinds (either member for k = 2, symmetric distances); LOO distances for Euclid kinds equal brute-force recomputation

### Implementation

- [X] T024 [US2] LOO distances, extent, margin (via index), coverage, quality, objectives v1 / v2 with switches `loo`, `margin`, `coverage`, `scale` in `crates/core/src/fitness.rs`; keep every v1-only branch behind `model: v1` so it can be deleted in one place (T055)
- [X] T025 [US2] `Raw` extension, `evaluate` delegating to `fitness`, medoid for DTW kinds in `crates/core/src/motif.rs`
- [X] T026 [P] [US2] `Kind::Zdtw`, CID, `min_std_rel` feasibility (clear error when no feasible window) in `crates/core/src/distance.rs` and `crates/core/src/motif.rs`
- [X] T027 [US2] New motif fields through `crates/core/src/result.rs`, `crates/py/src/lib.rs`, `python/ditscover/__init__.py`

**Checkpoint**: `cargo test fitness` green; quick synthetic run shows precision ≫ 0.15

---

## Phase 5: User Story 3 - Search that aligns boundaries and starts well (P1)

**Goal**: refinement, collective moves, guarded variation, split, NN seeding, archive, better stopping.

**Independent Test**: refinement recovers planted boundaries; seeds hit planted occurrences; runs honour `min_generations` (quickstart §3).

### Tests (first)

- [X] T028 [P] [US3] `crates/core/tests/refine.rs`: occurrences shifted / resized ≤ 15 % recover planted boundaries ± 2 points on noise-free data; refinement never worsens the member distance
- [X] T029 [P] [US3] `crates/core/tests/invariants.rs`: feature-001 invariants hold after `phase`, `rescale`, `split`, guarded `add` / `remove`, donation with mating restriction, `refine`, nearest-neighbour seeds and immigrants; guarded add never increases extent
- [X] T030 [P] [US3] `crates/core/tests/archive.rs`: archive bounded, distinct (containment part of the FR-024 rule; shape dedupe happens in the final selection), mutually non-dominated; archive HV non-decreasing while nothing is evicted
- [X] T031 [P] [US3] `crates/core/tests/ops.rs`: NN seeds on a planted series contain planted occurrences; gap cut returns the planted count on clean data; split separates a set mixing two shapes
- [X] T032 [P] [US3] `crates/core/tests/stop.rs`: no stagnation stop before `min_generations`; immigrants injected after half a stagnation window; immigrant windows not covered by the archive

### Implementation

- [X] T033 [US3] `refine` (coordinate descent, Lamarckian for archive entrants) in `crates/core/src/ops.rs`
- [X] T034 [P] [US3] `phase` and `rescale` collective mutations in `crates/core/src/ops.rs`
- [X] T035 [US3] Guarded `add` / `remove` / donation and mating restriction (`crates/core/src/ops.rs`, parent choice in `crates/core/src/engine.rs`)
- [X] T036 [P] [US3] `split` (2-medoids) in `crates/core/src/ops.rs`
- [X] T037 [US3] NN seeding with gap cut and initial mix (`seed_nn`) in `crates/core/src/ops.rs` / `crates/core/src/engine.rs`
- [X] T038 [US3] `Archive` in `crates/core/src/archive.rs`; HV and stagnation on archive; archive size / HV in `GenerationRecord` (`crates/core/src/events.rs`)
- [X] T039 [US3] `min_generations`, relative `stagnation_delta`, immigrants in `crates/core/src/engine.rs`

**Checkpoint**: quickstart §3; synthetic recall improves over Phase 4

---

## Phase 6: User Story 4 - Distinct motifs and conflict-free labelling (P2)

**Goal**: K distinct motifs ranked by quality and a labelling with no overlapping occurrences.

**Independent Test**: electric top motifs differ in shape; 0 overlapping occurrences; labels in result and report (quickstart §4).

### Tests (first)

- [X] T040 [P] [US4] `crates/core/tests/label.rs`: weighted interval DP equals brute force (proptest, ≤ 12 intervals); motifs below `min_occ` dropped and replaced by the next candidate; fewer than K motifs returned when candidates run out; labels vector consistent with occurrences
- [X] T041 [P] [US4] `crates/core/tests/result.rs`: `rank_by: quality` never ranks a set with margin ≤ 0 above one with margin > 0; containment and lag-tolerant shape dedupe (two phase-identical shapes → one kept); `top_k: auto` returns only positive-margin motifs and may return none; `series.csv` label column round-trips
- [X] T042 [P] [US4] `tests/python/test_report.py` / `tests/python/test_api.py`: `Result.labels` length = series length; labels band in the run report; new table columns sorted by quality

### Implementation

- [X] T043 [US4] Weighted interval scheduling and labels in `crates/core/src/label.rs`
- [X] T044 [US4] Quality ranking, dedupe v2, labelling loop, `top_k: auto`, label column in `crates/core/src/result.rs`
- [X] T045 [US4] `labels` to numpy (the binding passes `RunResult` through as is, no change needed in `crates/py/src/lib.rs`); `Result.labels` in `python/ditscover/__init__.py`
- [X] T046 [US4] Labels band and table columns in `python/ditscover/plot.py` / `python/ditscover/report.py`

**Checkpoint**: quickstart §4

---

## Phase 7: User Story 5 - Ablation study and new defaults (P2)

**Goal**: one benchmark decides, on validation only, which components stay; losers and (if beaten) v1 are deleted.

**Independent Test**: `contracts/ablation.example.yaml` produces the Ablation tab and `decisions`; `contracts/final.example.yaml` gives the test-split comparison (quickstart §5–§6).

### Tests (first)

- [X] T047 [P] [US5] `tests/python/test_perf.py` (`@pytest.mark.slow`): library defaults, one seed, all cores, 20 000-point synthetic series with planted motifs ⇒ median of 5 seeds < 10 s (SC-005, research R7)
- [X] T048 [P] [US5] `crates/core/benches/ga.rs`: `v2_defaults_20k` bench (defaults, 20 000 points) and v2 at 5 000 points
- [X] T049 [P] [US5] `tests/python/test_bench.py`: ablation rows `{component, dataset, split, delta_f1, ci_low, ci_high, p, p_holm}` computed when `full` and `full-*` labels exist, with seeds averaged per series and Holm across the components within each `(dataset, split)`; `decisions` from validation rows only: keep iff p_holm < 0.05 with F1 drop on ≥ 1 dataset and no significant rise on any, otherwise delete (including no significant effect), except `labelling`, always `keep: true` with reason "output guarantee"; changing test rows never changes a decision; `component: v1` row compares `full` vs `v1` with the same rule; Ablation tab renders ΔF1 per component × dataset × split and the decisions

### Implementation

- [X] T050 [US5] Run T047 / T048 on the full algorithm; if over budget, lower the default `min_generations` first, then `population`, in `crates/core/src/config.rs`; record the chosen defaults in `specs/003-motif-quality/research.md` (R7)
- [X] T051 [US5] Ablation rows, Holm within `(dataset, split)` and `decisions` in `python/ditscover/bench/__init__.py`; Ablation tab in `python/ditscover/report.py` / `python/ditscover/plot.py`
- [X] T052 [US5] Run `specs/003-motif-quality/contracts/ablation.example.yaml` (both splits, 30 series, 19 440 runs, ~11 h) to `results/ablation-003`; copy the validation `decisions` (per component: ΔF1 per dataset, Holm p, keep / delete; plus v1) to `specs/003-motif-quality/research.md` (R8) and note test ΔF1 that disagree
- [X] T053 [US5] Delete every component with `keep: false`, except `labelling` (output guarantee, FR-025), with its switch, config leaf, tests and `contracts/ablation.example.yaml` row (`crates/core/src/*.rs`, `crates/core/tests/*.rs`, `crates/core/src/config.rs`); update defaults
- [X] T054 [US5] Re-run pruned `full` on the validation rows of `contracts/ablation.example.yaml` and compare it with both `v1` and the unpruned `full` from T052 (same series and seeds); if pruned is significantly worse than unpruned on any dataset, restore the deleted component with the largest validation ΔF1 and repeat; apply the decision rule to `full` vs `v1`; record both comparisons in `specs/003-motif-quality/research.md` (R8)
- [X] T055 [US5] If `full` wins: delete `model: v1`, `rank_by: ideal` (only if `quality_rank` was kept), `dedupe_jaccard` (only if `dedupe_v2` was kept) and the v1-only branches (random `remove`, support / length objectives) from `crates/core/src/fitness.rs`, `crates/core/src/ops.rs`, `crates/core/src/result.rs`, `crates/core/src/config.rs`; delete the v1 golden test (T003) and the `v1` row of `contracts/ablation.example.yaml`; remove `model="v1"` from `specs/003-motif-quality/contracts/python-api.md`. If `full` loses: delete the v2 components instead and record it
- [X] T056 [US5] Run `specs/003-motif-quality/contracts/final.example.yaml` (pruned defaults vs tuned baselines, test split once) to `results/bench-003`; check SC-001..SC-004, SC-006; assert that no run in `results/bench-003/sets.jsonl` has overlapping occurrences across its motifs (SC-004); record the outcome honestly in `specs/003-motif-quality/research.md`

**Checkpoint**: quickstart §5; `decisions` all applied; no switch left without a `keep: true`

---

## Phase 8: Polish

- [X] T057 Re-run T047 / T048 on the pruned defaults (SC-005) and confirm median run time below LoCoMotif's per dataset in `results/bench-003`; compare with `cargo bench -p ditscover-core --bench ga -- --baseline pre-003`; any bench slower by more than 10 % gets a justification row in `specs/003-motif-quality/plan.md` Complexity Tracking or blocks the merge (Constitution VI)
- [X] T058 [P] Update `configs/*.yaml` and `README.md` examples to the final defaults (English, minimal) — configs done; `README.md` is being rewritten by the user and was left untouched
- [X] T059 Quickstart end to end (`specs/003-motif-quality/quickstart.md`); `cargo fmt` / `cargo clippy --all-targets -- -D warnings` / `cargo test` / `ruff check` / `ruff format` / `pytest` clean

---

## Dependencies & Execution Order

- Setup → Foundational → US1 baseline (T019) must be recorded before the ablation (T052).
- US2 and US3 depend on Foundational (index, null scale); US3's guarded operators and refinement use
  US2's LOO distances (T024 before T033–T035).
- US4 depends on US2 (margin, quality) and US3 (archive).
- US5 depends on US1–US4. Within US5: T050 (defaults fit the budget) → T052 (ablation) → T053
  (prune) → T054 (v1 decision) → T055 (v1 / v2 deletion) → T056 (final test run).

## Parallel Examples

```text
Phase 2: T003 config.rs | T004 index.rs | T005 fitness.rs   then T007 ‖ T008
Phase 3: T010 | T011 | T012 | T013                           then T015 ‖ T014
Phase 4: T020 | T021 | T022 | T023                           then T026 ‖ T024
Phase 5: T028 | T029 | T030 | T031 | T032                    then T034 ‖ T036 (after T033)
Phase 6: T040 | T041 | T042
Phase 7: T047 | T048 | T049
```

## Implementation Strategy

1. MVP = US1: measure correctly — no algorithm change is judged against the old, distorted numbers.
2. US2 → quick synthetic check (precision); US3 → recall; US4 → precision via conflict-free output.
3. US5: fit defaults to the 10 s budget, then keep only what earns its place on validation; delete
   the rest (and v1 if beaten); final test-split comparison once.

## Traceability

| Requirement | Tasks |
|---|---|
| FR-001–003 | T010, T014, T015 |
| FR-004 | T011, T013, T015–T017 |
| FR-005 | T011, T017 |
| FR-006 | T012, T018 |
| FR-007 | T011, T017 |
| FR-008–011 | T020, T021, T023, T024, T025 |
| FR-012–013 | T020, T022, T026 |
| FR-014 | T003, T024, T054, T055 |
| FR-015 | T028, T033 |
| FR-016 | T029, T034 |
| FR-017–018 | T029, T035 |
| FR-019 | T031, T036 |
| FR-020 | T031, T037 |
| FR-021 | T030, T038 |
| FR-022 | T032, T039 |
| FR-023–026 | T040–T046 |
| FR-027 | T049, T052 |
| FR-028 | T049, T051, T052 |
| FR-029 | T049, T052, T053 |
| SC-001–004, SC-006 | T056 |
| SC-005 | T047, T048, T050, T057 |
| SC-007 | T003, T004, T005 |
