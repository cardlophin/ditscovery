# Implementation Plan: Motif quality — fitness v2, smarter search, conflict-free labelling and ablation

**Branch**: `003-motif-quality` | **Date**: 2026-10-13 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/003-motif-quality/spec.md`

## Summary

First make the benchmark trustworthy (STOMP cut-off, LoCoMotif adapter, shared stretch window,
validation tuning, stored sets, error view, paired tests) and record a corrected baseline. Then
replace the fitness with a "closed set" model — leave-one-out member distances, extent, margin to
the nearest outsider, coverage, random-pair normalisation — backed by a per-run candidate index
(PAA sketches) that also powers additions, nearest-neighbour seeding and immigrants. Add boundary
refinement (memetic), collective moves, guarded additions, mating restriction, split, an archive and
better stopping. Finish with quality ranking, containment + shape deduplication and conflict-free
labelling. Every component is switchable; an ablation benchmark decides what stays, and losing
components are deleted. Formulas in [research.md](research.md).

## Technical Context

**Language/Version**: Rust 1.85+ (edition 2024); Python ≥ 3.12

**Primary Dependencies**: no new Rust crates; Python bench extra declares `scipy` (already
transitive through `stumpy`)

**Storage**: files (`sets.jsonl`, extended `summary.json`, `series.csv` with labels)

**Testing**: `cargo test` + `proptest` (fitness monotonicity, refinement, labelling DP vs brute
force, index exactness), `pytest` (adapters, tuning, stats, report tabs)

**Target Platform**: Linux, macOS, Windows (as before)

**Project Type**: library + CLI (feature on top of 001–002)

**Performance Goals**: median run time < LoCoMotif on every dataset; library defaults (with
`min_generations`), one seed, all cores, 20 000 points ⇒ median < 10 s (research R7); candidate
index ≤ ~1 MB at 20 000 points; ablation study ≈ 18 360 runs, ~10 h sequential (research R5)

**Constraints**: determinism by seed (index, null scale and seeding all seeded); v1 behaviour
reproducible bit-for-bit with `model: v1` and components off until the ablation removes it;
ablation decisions use validation only, 30 series per dataset, Holm within each dataset; a
component without a significant validation benefit is deleted, and v1 is deleted if `full` wins

**Scale/Scope**: series ≤ 2·10⁴ points in benchmarks; k ≤ 20 occurrences per set

## Constitution Check

| Principle | Compliance |
|---|---|
| I. Pure Rust core | fitness, index, operators, archive, ranking, dedupe, labelling in `crates/core`; Python: adapters, tuning loop, statistics, report ✅ |
| II. Friendly Python API | defaults improve silently; `res.labels`, new motif fields; `top_k="auto"` ✅ |
| III. Minimal code | no new crates; ablation switches are temporary by design (FR-029 deletes losers) — see Complexity Tracking ✅ |
| IV. Test-first | behavioural tests per acceptance scenario before code; proptest on invariants ✅ |
| V. Reproducibility | all new randomness from the `(seed, generation, index)` streams; null scale and seeding seeded ✅ |
| VI. Performance | candidate index with exact pruning; refinement cost bounded; benches extended ✅ |
| VII. Observability | `GenerationRecord` adds archive size and archive HV; no blocking ✅ |
| VIII. Single configuration | new leaves in the same `Config`; bench `grid`/`tune` in `BenchConfig` ✅ |

Post-design re-check (after the clarifications of 2026-10-13): no violations beyond the tracked
ones; the strict keep rule and the v1 deletion tighten Principle III; the 20k defaults bench backs
Principle VI.

## Project Structure

### Documentation (this feature)

```text
specs/003-motif-quality/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/ (config.v2.example.yaml, ablation.example.yaml, final.example.yaml, python-api.md, outputs.md)
└── tasks.md
```

### Source Code (repository root)

```text
crates/core/src/
├── index.rs        # CandidateIndex: PAA sketches, nearest outsider, NN pairs (new)
├── fitness.rs      # NullScale, LOO distances, extent, margin, coverage, quality, objectives v1/v2 (new)
├── archive.rs      # bounded archive of distinct non-dominated motifs (new)
├── label.rs        # weighted interval scheduling, labels vector (new)
├── motif.rs        # Raw extended; medoid prototype for DTW kinds; evaluate delegates to fitness
├── distance.rs     # Kind::Zdtw, CID correction
├── ops.rs          # refine, phase, rescale, guarded add/remove/donation, mating, split, nn seeding, immigrants
├── nsga.rs         # survive unchanged API; dedupe helpers (containment, shape)
├── engine.rs       # index + null scale build, archive, min_generations, immigrants, records
├── result.rs       # rank_by quality, dedupe v2, labelling, labels in series.csv
├── config.rs       # fitness section, new ga/stop/output leaves, validation
└── bench.rs        # tune, grid, max_runs, stats, save_sets, synthetic split, (name, split) keys
crates/core/tests/ (fitness.rs, index.rs, refine.rs, archive.rs, label.rs, + existing)
crates/core/benches/ga.rs   # + v2 timings at 5k; v2_defaults_20k (SC-005)
crates/py/src/lib.rs        # labels → numpy; new motif fields
python/ditscover/
├── __init__.py             # Result.labels, Motif new fields
├── bench/__init__.py       # tuning loop, sets.jsonl, stats (Wilcoxon + Holm), ablation table + decisions
├── bench/methods.py        # STOMP cutoff, LoCoMotif fix, ditscover stretch window
├── bench/datasets.py       # synthetic validation split, max_stretch from GT
├── plot.py                 # labels band, errors view, ablation chart
└── report.py               # Errors / Ablation tabs, new table columns
tests/python/ (test_methods.py, test_bench.py, test_report.py, test_api.py — extended;
               test_perf.py new, `slow` marker excluded by default in pyproject.toml)
```

**Structure Decision**: four new focused core modules (`index`, `fitness`, `archive`, `label`)
instead of growing `motif.rs` / `ops.rs`; Python changes stay in the existing benchmark and report
modules.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| ~15 boolean / probability switches for components | the ablation must measure each component in isolation (FR-027) | changing code per experiment is not reproducible; switches of losing components are deleted after the study (FR-029) |
| `model: v1` kept temporarily | baseline variant in the same binary for paired comparison; deleted with `rank_by: ideal` and Jaccard dedupe if `full` wins (FR-014) | a separate build would break the same-seed pairing |
