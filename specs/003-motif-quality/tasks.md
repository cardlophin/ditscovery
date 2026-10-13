---
description: "Task list: motif quality (fitness v2, search, labelling, ablation)"
---

# Tasks: Motif quality — fitness v2, smarter search, conflict-free labelling and ablation

**Input**: Design documents from `/specs/003-motif-quality/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: mandatory (Constitution IV). Each test block is written first and MUST fail.

## Format: `[ID] [P?] [Story] Description`

---

## Phase 1: Setup

- [ ] T001 Declare `scipy` in the `bench` extra (`pyproject.toml`); add empty modules `index`, `fitness`, `archive`, `label` to `crates/core/src/lib.rs`
- [ ] T002 Spike (not committed): run LoCoMotif on 3 synthetic series with stored outputs; compare found lengths / coordinates with GT; record the cause and the fix in `research.md` (R1, "LoCoMotif diagnosis")

---

## Phase 2: Foundational

**⚠️ Blocks US2–US4**

### Tests (first)

- [ ] T003 [P] `crates/core/tests/config.rs`: new leaves and defaults (data-model.md), validation errors with key path, flat overrides (`refine=0`, `top_k=auto`, `kind=zdtw`); golden test: `model: v1` + all components off reproduces the feature-001 result for a fixed seed bit for bit
- [ ] T004 [P] `crates/core/tests/index.rs`: nearest outsider equals brute force for same-length Euclid-type queries (proptest); excluded windows never returned; deterministic build; memory bound at 20 000 points
- [ ] T005 [P] `crates/core/src/fitness.rs` (`#[cfg(test)]`): `NullScale` deterministic, positive, interpolates between grid lengths

### Implementation

- [ ] T006 New `fitness` section and `ga` / `stop` / `output` leaves with validation in `crates/core/src/config.rs`
- [ ] T007 [P] `CandidateIndex` (PAA sketches, LB-ordered scan, refinement) in `crates/core/src/index.rs`
- [ ] T008 [P] `NullScale` in `crates/core/src/fitness.rs`
- [ ] T009 Build index and null scale once per run and expose them in `Ctx` (`crates/core/src/motif.rs`, `crates/core/src/engine.rs`)

**Checkpoint**: T003–T005 green; v1 golden test green

---

## Phase 3: User Story 1 - Trustworthy comparison and error analysis (P1) 🎯 MVP

**Independent Test**: corrected baseline `results/bench-002` with stored sets, Errors tab and paired tests.

### Tests (first)

- [ ] T010 [P] [US1] `tests/python/test_methods.py`: STOMP returns > 1 set when several motifs exist; LoCoMotif hits a planted motif with matching coordinates (per T002); ditscover receives `max_stretch` covering the GT stretch
- [ ] T011 [P] [US1] `tests/python/test_bench.py`: grid tuned on validation picks the best params of a fake method and evaluates test once; `max_runs` guard refuses oversized grids; synthetic validation suite differs from test; `sets.jsonl` written and resume-safe; Wilcoxon + Holm on a hand-made table
- [ ] T012 [P] [US1] `tests/python/test_report.py`: Errors tab marks hits, false alarms and misses for a hand-made series
- [ ] T013 [P] [US1] `crates/core/tests/bench.rs`: `tune`, `grid`, `max_runs`, `stats`, `save_sets` parse and validate

### Implementation

- [ ] T014 [US1] STOMP `cutoff=np.inf`, LoCoMotif fix from T002, ditscover stretch window in `python/ditscover/bench/methods.py`
- [ ] T015 [P] [US1] Synthetic validation split (`seed + 1_000_000`) and `max_stretch` from GT in `python/ditscover/bench/datasets.py`
- [ ] T016 [US1] `tune`, `grid`, `max_runs`, `stats`, `save_sets` in `crates/core/src/bench.rs`
- [ ] T017 [US1] Tuning loop, `sets.jsonl`, paired tests (`scipy.stats.wilcoxon`, Holm) and `tuned`/`tests` in `summary.json` in `python/ditscover/bench/__init__.py`
- [ ] T018 [US1] Errors view in `python/ditscover/plot.py` and Errors tab in `python/ditscover/report.py`
- [ ] T019 [US1] Run the corrected baseline to `results/bench-002` (002 example config + Fungi, Plane, Mallat); record per-dataset F1 / precision / recall in `research.md` ("Corrected baseline")

**Checkpoint**: baseline recorded; quickstart §1

---

## Phase 4: User Story 2 - Fitness that rewards closed motif sets (P1)

**Independent Test**: hand-built series behave as in the acceptance scenarios; synthetic precision rises.

### Tests (first)

- [ ] T020 [P] [US2] `crates/core/tests/fitness.rs`: true set dominates set + junk occurrence; margin(true) > margin(true − one); tiling dominated; k = 2 member distances not halved; flat / noise windows infeasible with `min_std_rel`; normalised objectives in [0, 1]
- [ ] T021 [P] [US2] Proptest in `crates/core/tests/fitness.rs`: adding an occurrence farther than the extent never improves f2 or f3
- [ ] T022 [P] [US2] `crates/core/src/distance.rs` tests: `zdtw` invariant to level and amplitude; CID factor ≥ 1 and = 1 for equal complexity
- [ ] T023 [P] [US2] `crates/core/tests/motif.rs`: medoid prototype for DTW kinds; LOO distances for Euclid kinds equal brute-force recomputation

### Implementation

- [ ] T024 [US2] LOO distances, extent, margin (via index), coverage, quality, objectives v1 / v2 with switches `loo`, `margin`, `coverage`, `scale` in `crates/core/src/fitness.rs`
- [ ] T025 [US2] `Raw` extension, `evaluate` delegating to `fitness`, medoid for DTW kinds in `crates/core/src/motif.rs`
- [ ] T026 [P] [US2] `Kind::Zdtw`, CID, `min_std_rel` feasibility in `crates/core/src/distance.rs` and `crates/core/src/motif.rs`
- [ ] T027 [US2] New motif fields through `crates/core/src/result.rs`, `crates/py/src/lib.rs`, `python/ditscover/__init__.py`

**Checkpoint**: `cargo test fitness` green; quick synthetic run shows precision ≫ 0.15

---

## Phase 5: User Story 3 - Search that aligns boundaries and starts well (P1)

**Independent Test**: refinement recovers planted boundaries; seeds hit planted occurrences; runs honour `min_generations`.

### Tests (first)

- [ ] T028 [P] [US3] `crates/core/tests/refine.rs`: occurrences shifted / resized ≤ 15 % recover planted boundaries ± 2 points on noise-free data; refinement never worsens the member distance
- [ ] T029 [P] [US3] `crates/core/tests/invariants.rs`: feature-001 invariants hold after `phase`, `rescale`, `split`, guarded `add` / `remove`, donation with mating restriction; guarded add never increases extent
- [ ] T030 [P] [US3] `crates/core/tests/archive.rs`: archive bounded, distinct (FR-024 rule), mutually non-dominated; archive HV non-decreasing while nothing is evicted
- [ ] T031 [P] [US3] `crates/core/tests/ops.rs`: NN seeds on a planted series contain planted occurrences; gap cut returns the planted count on clean data; split separates a set mixing two shapes
- [ ] T032 [P] [US3] `crates/core/tests/stop.rs`: no stagnation stop before `min_generations`; immigrants injected after half a stagnation window; immigrant windows not covered by the archive

### Implementation

- [ ] T033 [US3] `refine` (coordinate descent, Lamarckian for archive entrants) in `crates/core/src/ops.rs`
- [ ] T034 [P] [US3] `phase` and `rescale` collective mutations in `crates/core/src/ops.rs`
- [ ] T035 [US3] Guarded `add` / `remove` / donation and mating restriction (`crates/core/src/ops.rs`, parent choice in `crates/core/src/engine.rs`)
- [ ] T036 [P] [US3] `split` (2-medoids) in `crates/core/src/ops.rs`
- [ ] T037 [US3] NN seeding with gap cut and initial mix (`seed_nn`) in `crates/core/src/ops.rs` / `engine.rs`
- [ ] T038 [US3] `Archive` in `crates/core/src/archive.rs`; HV and stagnation on archive; archive size / HV in `GenerationRecord` (`crates/core/src/events.rs`)
- [ ] T039 [US3] `min_generations`, relative `stagnation_delta`, immigrants in `crates/core/src/engine.rs`

**Checkpoint**: quickstart §3; synthetic recall improves over Phase 4

---

## Phase 6: User Story 4 - Distinct motifs and conflict-free labelling (P2)

**Independent Test**: electric top motifs differ in shape; 0 overlapping occurrences; labels in result and report.

### Tests (first)

- [ ] T040 [P] [US4] `crates/core/tests/label.rs`: weighted interval DP equals brute force (proptest, ≤ 12 intervals); motifs below `min_occ` dropped and replaced by the next candidate; labels vector consistent with occurrences
- [ ] T041 [P] [US4] `crates/core/tests/result.rs`: `rank_by: quality` never ranks a set with margin ≤ 0 above one with margin > 0; containment and lag-tolerant shape dedupe (two phase-identical shapes → one kept); `top_k: auto`; `series.csv` label column round-trips
- [ ] T042 [P] [US4] `tests/python/test_report.py` / `test_api.py`: `Result.labels` length = series length; labels band in the run report; new table columns sorted by quality

### Implementation

- [ ] T043 [US4] Weighted interval scheduling and labels in `crates/core/src/label.rs`
- [ ] T044 [US4] Quality ranking, dedupe v2, labelling loop, `top_k: auto`, label column in `crates/core/src/result.rs`
- [ ] T045 [US4] `labels` to numpy in `crates/py/src/lib.rs`; `Result.labels` in `python/ditscover/__init__.py`
- [ ] T046 [US4] Labels band and table columns in `python/ditscover/plot.py` / `python/ditscover/report.py`

**Checkpoint**: quickstart §4

---

## Phase 7: User Story 5 - Ablation study and new defaults (P2)

**Independent Test**: one command produces the ablation table; decisions recorded; defaults pruned.

- [ ] T047 [P] [US5] `tests/python/test_bench.py`: ablation rows (ΔF1, bootstrap 95 % CI, Holm p) computed when `full` and `full-*` labels exist; Ablation tab rendered
- [ ] T048 [US5] Ablation summary in `python/ditscover/bench/__init__.py`; Ablation tab in `python/ditscover/report.py` / `plot.py`
- [ ] T049 [US5] Run `contracts/ablation.example.yaml` on the validation split to `results/ablation-003`; apply the decision rule (research R5); record decisions in `research.md` (R7)
- [ ] T050 [US5] Delete losing components and their switches (code, config leaves, tests); update defaults; remove `model: v1` if v2 wins (keep a tagged commit of the last v1 for reference)
- [ ] T051 [US5] Final test-split benchmark of pruned `ditscover` vs tuned baselines to `results/bench-003`; check SC-001..SC-005 and record outcome in `research.md`

---

## Phase 8: Polish

- [ ] T052 [P] Extend `crates/core/benches/ga.rs` with v2 at 5 000 / 20 000 points (SC-005)
- [ ] T053 [P] Update `configs/*.yaml` and `README.md` examples to the final defaults (English, minimal)
- [ ] T054 Quickstart end to end; `cargo fmt` / `clippy -D warnings` / `ruff` / `pytest` clean

---

## Dependencies & Execution Order

- Setup → Foundational → US1 baseline (T019) must be recorded before the ablation (T049).
- US2 and US3 depend on Foundational (index, null scale); US3's guarded operators and refinement use
  US2's LOO distances (T024 before T033–T035).
- US4 depends on US2 (margin, quality) and US3 (archive).
- US5 depends on US1–US4.

## Parallel Example: Phase 5

```text
T028 refine.rs | T029 invariants.rs | T030 archive.rs | T031 ops.rs | T032 stop.rs
then: T034 phase/rescale ‖ T036 split (after T033)
```

## Implementation Strategy

1. US1: measure correctly — no algorithm change is judged against the old, distorted numbers.
2. US2 → quick synthetic check (precision); US3 → recall; US4 → precision via conflict-free output.
3. US5: keep only what earns its place; final test-split comparison once.

## Traceability

| FR | Tasks |
|---|---|
| FR-001–003 | T010, T014, T015 |
| FR-004 | T011, T013, T015–T017 |
| FR-005 | T011, T017 |
| FR-006 | T012, T018 |
| FR-007 | T011, T017 |
| FR-008–011 | T020, T021, T023, T024, T025 |
| FR-012–013 | T020, T022, T026 |
| FR-014 | T003, T024 |
| FR-015 | T028, T033 |
| FR-016 | T029, T034 |
| FR-017–018 | T029, T035 |
| FR-019 | T031, T036 |
| FR-020 | T031, T037 |
| FR-021 | T030, T038 |
| FR-022 | T032, T039 |
| FR-023–026 | T040–T046 |
| FR-027–029 | T047–T051 |
