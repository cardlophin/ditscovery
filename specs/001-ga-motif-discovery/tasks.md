---

description: "Task list: evolutionary discovery of variable-length motifs"
---

# Tasks: Evolutionary discovery of variable-length motifs

**Input**: Design documents from `/specs/001-ga-motif-discovery/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: mandatory (Constitution IV). Each test block is written first and MUST fail before
its implementation tasks start.

**Organization**: tasks are grouped by user story so each story can be implemented and tested on
its own.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished tasks)
- **[Story]**: user story from spec.md (US1–US5)
- Paths are relative to the repository root; `R#` refers to research.md sections

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: workspace, packaging and quality gates

- [X] T001 Turn `Cargo.toml` into a workspace (`crates/core`, `crates/py`, `resolver = "3"`, shared `[workspace.lints]` with clippy `-D warnings`); create `crates/core/Cargo.toml` (package `ditscover-core`) and `crates/py/Cargo.toml` (package `ditscover-py`, `crate-type = ["cdylib"]`) with exactly the dependencies of plan.md "Dependency justification" (no `polars`, no `anyhow`; `serde_yaml_ng = "0.10"`)
- [X] T002 Move `src/ts/time_series.rs` → `crates/core/src/series.rs` and `src/ts/loader.rs` → `crates/core/src/io.rs` (keep the builder API, switch to the `csv` crate per R13); create `crates/core/src/lib.rs` declaring the modules of plan.md; delete `src/` and `main.py`
- [X] T003 [P] Configure `pyproject.toml` for maturin (`[build-system] maturin`, `manifest-path = "crates/py/Cargo.toml"`, `python-source = "python"`, `module-name = "ditscover._core"`, runtime deps `numpy`, `plotly`, `[project.scripts] ditscover = "ditscover.cli:main"`, optional extra `dev = [maturin, pytest, ruff, pandas]`, `[tool.ruff]`) and create an empty `python/ditscover/__init__.py`
- [X] T004 [P] Quality gates: `rustfmt.toml` and `.github/workflows/ci.yml` running `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `uv pip install -e '.[dev]'`, `maturin develop`, `ruff check`, `ruff format --check`, `pytest tests/python`

**Checkpoint**: `cargo build` and `uv run maturin develop` succeed on empty modules

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: config, series, loading, distances, events and synthetic data used by every story

**⚠️ CRITICAL**: no user story work can begin until this phase is complete

### Tests (write first, must fail)

- [X] T005 [P] Config tests in `crates/core/tests/config.rs`: all defaults of data-model.md; `deny_unknown_fields` error names the key; `validate()` errors carry the key path (e.g. `motif.max_len: must be >= min_len (12)`); `set_leaf` by unique leaf name (`min_len`, `dir`, `preprocess` as YAML value) and unknown leaf ⇒ error; precedence defaults < YAML < overrides; `validate_len(n, &cfg)` rejects `n < 2·min_len`
- [X] T006 [P] Loader tests in `crates/core/tests/io.rs`: column by name and by index, custom delimiter (TSV), no header, missing column ⇒ error listing available columns, non-numeric column, empty file, all-NaN column ⇒ explicit error, wrong delimiter ⇒ explicit error, `NaN` cells parsed as `f64::NAN`
- [X] T007 [P] Distance tests in `#[cfg(test)]` of `crates/core/src/distance.rs`: `resample` identity and endpoints, `znorm` of a flat stretch = zero vector, DTW with minimal band = Euclidean, DTW ≤ Euclidean, all functions write into caller-provided slices (signature `fn(&[f64], &mut [f64])`)
- [X] T008 [P] Event tests in `crates/core/tests/events.rs`: `try_send` on a full channel does not block and increments the dropped counter; with `output.dir` set the consumer writes `history.jsonl` with one valid JSON object per line matching contracts/outputs.md; `GenerationRecord` has every field of data-model.md and `front` is `Some` only every `snapshot_every` generations; `join` returns the records in order
- [X] T009 [P] Synthetic data tests in `#[cfg(test)]` of `crates/core/src/synth.rs`: planted occurrences inside the series and non-overlapping, lengths within `L·(1±s)`; `match_occurrences` with IoU ≥ 0.5 and one-to-one greedy matching gives the expected recall/precision on hand-made cases (R16)

### Implementation

- [X] T010 `Config` tree (data-model.md), defaults, `validate`, `validate_len`, `set_leaf`, YAML load/dump with `serde_yaml_ng`, and a `thiserror` error enum (`Config { path, msg }`, `Data`, `Io`) in `crates/core/src/config.rs`
- [X] T011 [P] `Series { values, origin, scale }` and `Stats` (prefix sums of `x` and `x²`, O(1) mean/std) in `crates/core/src/series.rs`
- [X] T012 [P] `Loader` trait + `CsvLoader` (name/index column, delimiter, header, NaN handling, errors of T006) in `crates/core/src/io.rs`
- [X] T013 [P] `Distance { ZEuclid, Euclid, Dtw { band } }`, `resample`, `znorm`, `dtw` (one-row Sakoe-Chiba) writing into caller slices, no allocation (R3, R17) in `crates/core/src/distance.rs`
- [X] T014 [P] `Event`, `GenerationRecord`, bounded channel with `try_send` + `AtomicU64` drop counter, consumer thread (optional `history.jsonl` in `output.dir`, `tracing::info!` every `log_every`, returns records on `join`) in `crates/core/src/events.rs`
- [X] T015 [P] `planted(n, motifs, occ, stretch, noise, seed)` and `match_occurrences(found, planted) -> (recall, precision)` in `crates/core/src/synth.rs`

**Checkpoint**: `cargo test -p ditscover-core` green for T005–T009

---

## Phase 3: User Story 1 - Discover motifs from a notebook (Priority: P1) 🎯 MVP

**Goal**: `ds.discover(...)` returns the top-K distinct motifs + Pareto front, deterministic,
interruptible with Ctrl+C.

**Independent Test**: planted motifs recovered (SC-001) and identical results with 1 vs 4 threads
(SC-003); Ctrl+C returns `stop == "cancelled"` within 1 s.

### Tests for User Story 1 (write first, must fail)

- [X] T016 [P] [US1] Motif evaluation tests in `crates/core/tests/motif.rs`: prototype of two known occurrences of different length equals the hand-computed resampled mean (FR-007); raw and normalised objectives on a known motif (R4); `aligned` rows have length `L*` and are z-normalised for `zeuclid`; Jaccard of coverage on known intervals; `evaluate` with a reused `Scratch` gives the same result as a fresh one
- [X] T017 [P] [US1] Proptest of the R2 invariants after `init`, `crossover`, each mutation and `repair` (including `min_len == max_len` and series with flat stretches) in `crates/core/tests/invariants.rs`
- [X] T018 [P] [US1] NSGA tests in `#[cfg(test)]` of `crates/core/src/nsga.rs`: ranking on known sets, infinite crowding at extremes, `hv3d` vs brute force on a grid, clearing by `niche_jaccard`
- [X] T019 [P] [US1] Result tests in `crates/core/tests/result.rs`: `top_k` ordered by (Pareto rank, distance to ideal) with index tie-break (FR-015); pairwise occurrence Jaccard ≤ `dedupe_jaccard`; fewer distinct motifs than K ⇒ returns what exists; occurrences mapped to original indices via `origin`/`scale`
- [X] T020 [P] [US1] Determinism in `crates/core/tests/determinism.rs`: same seed with Rayon pools of 1 and 4 threads ⇒ identical `RunResult` (excluding `elapsed_s`)
- [ ] T021 [P] [US1] Planted motifs in `crates/core/tests/planted.rs`: median over 10 seeds, recall ≥ 0.9 and precision ≥ 0.8 with IoU ≥ 0.5 on the top 5 (SC-001); `#[cfg_attr(debug_assertions, ignore)]`; plus white noise ⇒ returns motifs, never an error
- [X] T022 [P] [US1] `tests/python/test_api.py`: input as list / ndarray / `pd.Series` / CSV path; `ValueError` with key path for an invalid kwarg and for series < `2·min_len`; `OSError` for a missing file; attributes of `Result`/`Motif` per contracts/python-api.md (`aligned` ndarray for top-K, `None` in `front`); `res.series` equals the input; `front` not empty; same seed ⇒ same result; `_thread.interrupt_main()` from a `threading.Timer` ⇒ `stop == "cancelled"` within 1 s

### Implementation for User Story 1

- [X] T023 [US1] `Segment`, `Individual`, `Raw`, `Scratch`, `evaluate` (prototype, raw + normalised objectives, no allocation, R17), `align` (resampled occurrences for output), coverage and Jaccard in `crates/core/src/motif.rs`
- [X] T024 [US1] `repair` (R2), `init` (random + seeded, R6), donation `crossover` and `shift`/`stretch`/`add`/`remove` mutations (R7) in `crates/core/src/ops.rs` (depends on T023)
- [X] T025 [P] [US1] `nds`, `crowding`, `clearing`, `hv3d`, `survive` (R5, R8) in `crates/core/src/nsga.rs` (depends on T023)
- [X] T026 [US1] Engine loop in `crates/core/src/engine.rs`: RNG `ChaCha8Rng` from `splitmix64(seed, gen, idx)` (R10), variation + evaluation with `par_iter().map_init(Scratch::new, …)`, survival, `max_generations`, cancellation `AtomicBool` checked per generation, events via `try_send` (depends on T024, T025, T014)
- [X] T027 [US1] `top_k` (R9), mapping to original indices, `Motif` with `aligned` for top-K, `RunResult` including original `series` in `crates/core/src/result.rs`
- [X] T028 [US1] `pub fn discover(series, &Config, &AtomicBool) -> Result<RunResult>` running `Config::validate` → `validate_len` → engine (state `Prepared` before `Running`) in `crates/core/src/lib.rs`
- [X] T029 [US1] Binding `discover` in `crates/py/src/lib.rs`: zero-copy `PyReadonlyArray1<f64>`, `pythonize` for config/kwargs/result, engine in a separate thread with the GIL released, `py.check_signals()` every 100 ms setting the `AtomicBool` on `KeyboardInterrupt` and returning the partial result (R12), `Config`/`Data` errors → `ValueError`, `Io` → `OSError`, one-time `tracing-subscriber` init
- [X] T030 [US1] `discover`, `Result`, `Motif` (light dataclasses, `prototype`/`aligned`/`series` as ndarray, `np.asarray` for any array-like input) in `python/ditscover/__init__.py`

**Checkpoint**: T016–T022 green; quickstart §1 and §2 (without plot) pass

---

## Phase 4: User Story 2 - Declarative preprocessing (Priority: P2)

**Goal**: ordered preprocessing steps with positions always reported in original indices.

**Independent Test**: series with NaN + trend, `interpolate` + `detrend` + `paa: 4` ⇒ occurrences
point to the correct stretches of the original series.

### Tests for User Story 2 (write first, must fail)

- [X] T031 [P] [US2] Tests in `crates/core/tests/preprocess.rs`: each `Step` on a known input, composition in declared order, `origin`/`scale` after `drop`, `diff` and `paa`, `smooth`/`paa` window > current length ⇒ error with path `preprocess[i].<step>` raised before the engine runs (R19)
- [X] T032 [P] [US2] Case in `tests/python/test_api.py`: `preprocess=[{"nan": "interpolate"}, {"detrend": "linear"}, {"paa": 4}]` via kwargs on a series with gaps and a planted motif ⇒ occurrences in original indices overlap the planted ones (IoU ≥ 0.5)

### Implementation for User Story 2

- [X] T033 [US2] `Step` enum (data-model.md) and `apply(series, &[Step]) -> Result<Series>` calling `config::validate_step(i, &step, n)` before each step in `crates/core/src/preprocess.rs`; add `validate_step` to `crates/core/src/config.rs`
- [X] T034 [US2] Run preprocessing in `discover` between `validate` and `validate_len`, keeping the original series for `RunResult.series`, in `crates/core/src/lib.rs`

**Checkpoint**: US1 tests still green; T031–T032 green

---

## Phase 5: User Story 3 - CLI experiments with YAML (Priority: P2)

**Goal**: reproducible batch runs from a YAML file with argument overrides, saved to a
self-contained directory.

**Independent Test**: `ditscover run cfg.yaml --seed 7 --out res/` twice ⇒ identical
`results.json` motifs; `res/` holds the four files of contracts/outputs.md.

### Tests for User Story 3 (write first, must fail)

- [X] T035 [P] [US3] Round-trip tests in `crates/core/tests/result.rs`: `save(dir)` writes `results.json` (no `history`, no `series`), `history.jsonl`, `config.yaml`, `series.csv` (header `value`, `NaN` kept); `load(dir)` returns a `RunResult` equal to the original (NaN-aware comparison for `series`)
- [X] T036 [P] [US3] `tests/python/test_cli.py`: `run` writes the four files into `--out`; repeat ⇒ identical motifs; unknown key ⇒ exit 2 naming the key; out-of-range value ⇒ exit 2; `--set` and shortcuts win over YAML; data error ⇒ exit 1; summary table on stdout; `-q` silences stderr; `ds.load(dir)` restores `series` without the source file

### Implementation for User Story 3

- [X] T037 [US3] `save`/`load` of `RunResult` (`results.json` via `serde_json`, `config.yaml`, `series.csv` via `csv` writer and `CsvLoader`, `history.jsonl` rewritten from memory) and writing the result at the end of `discover` when `output.dir` is set, in `crates/core/src/result.rs` and `crates/core/src/lib.rs`
- [X] T038 [US3] Bindings `load_result(dir)`, `save_result(result, dir)` and config from YAML path + overrides (values parsed as YAML in Rust) in `crates/py/src/lib.rs`
- [X] T039 [US3] `Result.save(dir)` and `ds.load(dir)` in `python/ditscover/__init__.py`
- [X] T040 [US3] `ditscover run` (argparse; shortcuts `--data`→`path`, `--column`, `--seed`, `--out`→`dir` with default `./ditscover-out`; `--set KEY=VALUE`; summary table; exit codes 0/1/2/130 per contracts/cli.md) in `python/ditscover/cli.py`

**Checkpoint**: quickstart §3 (without `--plot`) passes

---

## Phase 6: User Story 4 - Visualise motifs and their evolution (Priority: P3)

**Goal**: interactive, self-contained, offline HTML with the four views.

**Independent Test**: from a saved directory, with the source data moved away, `ditscover plot DIR`
writes an HTML with the four views that renders with the network disabled.

### Tests for User Story 4 (write first, must fail)

- [X] T041 [P] [US4] `tests/python/test_plot.py`: trace count per view, one distinct colour per motif, `overlay` draws `aligned` rows + prototype (no resampling in Python), front slider has one frame per snapshot, HTML written; default HTML embeds plotly.js (no `cdn.plot.ly` URL) and `cdn=True` references the CDN; `ditscover plot DIR` works after the source CSV is deleted

### Implementation for User Story 4

- [X] T042 [US4] `motifs`, `overlay`, `evolution`, `front`, `report(result, path, cdn=False)` with `include_plotlyjs=True` by default (R15) in `python/ditscover/plot.py`
- [X] T043 [US4] `Result.plot(kind, path=None, cdn=False)` in `python/ditscover/__init__.py`; `ditscover plot DIR [--kind] [--out] [--cdn]` and `run --plot [--cdn]` in `python/ditscover/cli.py` (depends on T039, T040)

**Checkpoint**: quickstart §3 complete, including the offline plot

---

## Phase 7: User Story 5 - Monitor and control long runs (Priority: P3)

**Goal**: time budget, stagnation stop and visible progress; logging never slows the engine.

**Independent Test**: a 5 s budget finishes in ≤ 6 s with `stop == "time_budget"`; a slow sink
gives `dropped_events > 0` without slowing the run.

### Tests for User Story 5 (write first, must fail)

- [X] T044 [P] [US5] Tests in `crates/core/tests/stop.rs`: `time_budget_s` stops within budget + 1 s with `TimeBudget`; HV stagnation over `stagnation_window` with `stagnation_delta` ⇒ `Stagnation`; cancel before generation 0 ⇒ `motifs = []` and `Cancelled`; cancel mid-run ⇒ valid result from the last complete generation; slow consumer (capacity 1) ⇒ `dropped_events > 0` and runtime within 5 % of logging off
- [X] T045 [P] [US5] Case in `tests/python/test_cli.py`: `run` interrupted with SIGINT ⇒ exit 130 and partial result written; `-v` shows per-generation progress on stderr

### Implementation for User Story 5

- [X] T046 [US5] `time_budget_s` and HV stagnation stop criteria with `StopReason` in `crates/core/src/engine.rs`
- [X] T047 [US5] `-q`/`-v` mapped to `log.level`, SIGINT ⇒ write partial result and exit 130 in `python/ditscover/cli.py`

**Checkpoint**: all user stories green

---

## Phase 8: Polish & Cross-Cutting Concerns

- [X] T048 [P] Criterion benches in `crates/core/benches/ga.rs`: `ga/run_10k` (10⁴ points, population 200, 300 generations — SC-002), `ga/threads_1` and `ga/threads_8` (SC-006), `ga/log_on`/`ga/log_off` (SC-004), `distance/{zeuclid,dtw}` (hot loop)
- [X] T049 [P] `scripts/bench_check.py` (stdlib only): read `target/criterion/**/change/estimates.json` and exit 1 if any mean change > +10 % (SC-008, R20)
- [X] T050 Check that `evaluate` and `distance.rs` allocate nothing in the hot loop (no `Vec::new`/`collect`/`to_vec` reachable from `evaluate`); fix any found in `crates/core/src/motif.rs` / `crates/core/src/distance.rs`
- [X] T051 Run quickstart.md end to end on the reference machine (Apple M4), save the criterion baseline `ref`, record the SC-002/SC-004/SC-006 numbers in the PR description; if SC-006 < 5× on 4 performance cores, report it to revise the spec rather than over-optimising
- [X] T052 Final gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `ruff check`, `ruff format --check`, `pytest tests/python`; no function > 40 lines without justification

---

## Dependencies & Execution Order

- Setup (T001–T004) → Foundational (T005–T015) → US1 (T016–T030).
- US2 (T031–T034) and US3 (T035–T040) depend only on US1 and can run in parallel with each other.
- US4 (T041–T043) depends on US1 and on US3 (`save`/`load` for `ditscover plot DIR`).
- US5 (T044–T047) depends on US1; T045/T047 extend `cli.py`, so they go after T040.
- Polish (T048–T052) after the stories you plan to ship; T048 can start right after US1.
- Within each story: tests → `motif` model → operators/NSGA → engine → result → bindings → Python.
- Same-file sequencing: T007→T013 (`distance.rs`), T009→T015 (`synth.rs`), T018→T025 (`nsga.rs`),
  T019→T035 (`tests/result.rs`), T022→T032 (`test_api.py`), T036→T045 (`test_cli.py`),
  T040→T043→T047 (`cli.py`), T029→T038 (`crates/py/src/lib.rs`), T030→T039→T043 (`__init__.py`).

## Parallel Example: User Story 1

```text
Tests together:  T016 motif.rs | T017 invariants.rs | T018 nsga tests | T019 result.rs
                 | T020 determinism.rs | T021 planted.rs | T022 test_api.py
Then:            T023 motif.rs
Then together:   T024 ops.rs ‖ T025 nsga.rs
Then:            T026 → T027 → T028 → T029 → T030
```

## Parallel Example: after US1

```text
Developer A: US2 (T031–T034)      Developer B: US3 (T035–T040) → US4 (T041–T043)
Developer C: US5 core (T044, T046), then T045/T047 after T040
```

## Implementation Strategy

1. **MVP = Phases 1–3**: deterministic, interruptible discovery from Python, validated with
   planted motifs. Stop and validate quickstart §1–§2.
2. Add US2 (real data) → US3 (batch experiments, persistence) → US4 (interpretation) → US5
   (control). Each checkpoint leaves the library usable.
3. Benchmarks are written in Polish but can be run from US1 onward to catch regressions early.
