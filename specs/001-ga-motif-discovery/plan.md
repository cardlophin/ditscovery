# Implementation Plan: Evolutionary discovery of variable-length motifs

**Branch**: `001-ga-motif-discovery` | **Date**: 2026-10-03 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/001-ga-motif-discovery/spec.md`

## Summary

Rust engine that evolves motifs as lists of variable-length segments with NSGA-II (support,
cohesion, length), clearing by occurrence overlap to keep motifs distinct, operators with repair
that guarantee feasibility, deterministic parallel evaluation (Rayon + per-individual RNG,
allocation-free with per-worker scratch buffers) and event logging over a non-blocking channel.
Exposed to Python via PyO3 (one-call API with Ctrl+C support from the MVP, CLI and offline Plotly
reports). Results are self-contained directories that include the original series. Algorithm
details in [research.md](research.md).

## Technical Context

**Language/Version**: Rust 1.85+ (edition 2024; dev machine has 1.97); Python ≥ 3.12

**Primary Dependencies**: see [Dependency justification](#dependency-justification)

**Storage**: files per results directory (`results.json`, `history.jsonl`, `config.yaml`,
`series.csv`, optional `report.html`)

**Testing**: `cargo test` + `proptest`, `criterion`; `pytest`

**Target Platform**: Linux, macOS (arm64/x86_64), Windows; abi3 wheels

**Project Type**: library (Rust + Python extension) with CLI

**Performance Goals**: on the reference machine (Apple M4 4P+6E, 16 GB, macOS 26, `--release`):
10⁴ points, population 200, 300 generations < 30 s; ≥ 5× speed-up with 8 threads; logging
overhead < 5 %; on any machine, no reference bench regresses > 10 % against its saved baseline

**Constraints**: seed determinism independent of thread count; logging never blocks; stop
effective ≤ 1 s; no allocations in the distance/evaluation hot loop; HTML works offline

**Scale/Scope**: series up to ~10⁶ points; univariate; CSV/TSV only in v1

## Dependency justification

Constitution III: every dependency is justified here. ★ = listed in the constitution's base
dependencies.

| Crate / package | Where | Why |
|---|---|---|
| `rayon` ★ | core | parallel evaluation and variation (VI) |
| `rand` ★ + `rand_chacha` | core | `ChaCha8Rng` has a stable, documented stream across platforms and `rand` versions, which per-`(seed, gen, idx)` determinism needs (V); `StdRng` does not guarantee this |
| `rand_distr` | core | `Normal` for `shift`/`stretch` mutations; avoids a hand-written sampler |
| `serde` ★ | core | single serialisable `Config` and results (VIII) |
| `serde_json` | core, py | `results.json` and `history.jsonl` (feature `float_roundtrip` so saved results reload bit-exact); in py, the `Value` type for kwargs overrides |
| `serde_yaml_ng` ★ (YAML) | core | YAML config; maintained fork of the deprecated `serde_yaml` (R14) |
| `csv` | core | replaces `polars` (R13, see Complexity Tracking) |
| `crossbeam-channel` ★ | core | bounded channel with `try_send` (VII) |
| `tracing` ★ | core | diagnostic logs from the consumer thread (VII) |
| `thiserror` | core | typed errors (`Config`, `Data`, `Io`) that `crates/py` maps to `ValueError`/`OSError` (II) without hand-written `Display`; replaces the current `anyhow` |
| `pyo3` ★ (abi3-py312), `numpy` ★ | py | bindings, zero-copy input |
| `pythonize` | py | Config/Result ↔ dict without hand-written conversion code (III) |
| `tracing-subscriber` | py | prints `tracing` events to stderr; initialised once in the binding, so the core stays subscriber-free |
| `numpy`, `plotly` | Python runtime | arrays and HTML reports (constitution) |
| `proptest`, `criterion` | dev (Rust) | invariants (IV), benches (VI) |
| `pytest`, `ruff`, `maturin`, `pandas` | dev (Python) | tests, gates, build; `pandas` only to test `pd.Series` input |

Removed: `polars`, `anyhow`.

## Constitution Check

| Principle | Compliance |
|---|---|
| I. Pure Rust core | All algorithm in `crates/core`, including occurrence alignment for the overlay view (`Motif.aligned`); `crates/py` and `python/ditscover` only convert, delegate and draw ✅ |
| II. Friendly Python API | `ds.discover(...)` + `res.plot()`; kwargs = YAML leaf names; GIL released and Ctrl+C handled from the US1 binding; errors → `ValueError`/`OSError` with key path ✅ |
| III. Minimal code | Extension points only `Loader` (trait), `Distance`, `Step` (enums) and stop criteria (config struct + `StopReason` enum, no trait needed); every dependency justified above; `pythonize` avoids conversions ✅ |
| IV. Test-first | proptest invariants, planted motifs (SC-001), unit tests for `motif` evaluation and result round-trip, pytest for API/CLI — all before code ✅ |
| V. Reproducibility | RNG per `(seed, gen, idx)`; scratch buffers cannot affect results; effective config + version + series saved ✅ |
| VI. Performance | Rayon for variation+evaluation; prefix sums; per-worker `Scratch`, no allocation in `evaluate` (R17); criterion benches + `bench_check.py` > 10 % gate (R20) ✅ |
| VII. Observability | bounded channel `try_send` + consumer thread (JSONL + `tracing`) + dropped-event counter ✅ |
| VIII. Single config | `Config` serde; all validation in `config.rs` before the engine starts (R19); leaf overrides; defined precedence ✅ |

Post-design re-evaluation: one justified deviation (base dependency `polars` → `csv`), see
Complexity Tracking. Constitution and CLAUDE.md name the preprocessing extension point
differently (`Preprocess` vs `Step`); this plan follows CLAUDE.md (`Step`) — a constitution PATCH
should align the name.

## Project Structure

### Documentation (this feature)

```text
specs/001-ga-motif-discovery/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/ (python-api.md, cli.md, outputs.md, config.example.yaml)
└── tasks.md
```

### Source Code (repository root)

```text
Cargo.toml                 # workspace
pyproject.toml             # maturin: manifest crates/py, python-source python, script ditscover, extra dev
crates/core/
├── src/
│   ├── lib.rs             # discover(series, &Config, &AtomicBool) -> Result<RunResult>
│   ├── config.rs          # Config, defaults, validate, validate_step, validate_len, set_leaf
│   ├── series.rs          # Series, Stats, origin map
│   ├── io.rs              # Loader trait, CsvLoader (migrated from src/ts/loader.rs)
│   ├── preprocess.rs      # Step + apply
│   ├── distance.rs        # Distance, resample, znorm, dtw (slice in, slice out)
│   ├── motif.rs           # Segment, Individual, Scratch, evaluate, prototype, align, jaccard
│   ├── ops.rs             # init, crossover, mutate, repair
│   ├── nsga.rs            # nds, crowding, clearing, hv3d
│   ├── engine.rs          # loop, rng, stopping
│   ├── events.rs          # Event, channel, consumer
│   ├── result.rs          # RunResult, top_k, save/load (incl. series.csv)
│   └── synth.rs           # series with planted motifs + IoU metrics
├── tests/ (invariants.rs, determinism.rs, planted.rs, config.rs, io.rs, preprocess.rs,
│           events.rs, motif.rs, result.rs, stop.rs)
└── benches/ga.rs
crates/py/src/lib.rs       # discover (threaded, signal polling), load_result, synthetic
python/ditscover/
├── __init__.py            # discover, load, Result, Motif
├── plot.py                # motifs, overlay, evolution, front, report
└── cli.py                 # run, plot
scripts/bench_check.py     # fails on > 10 % criterion regression
tests/python/ (test_api.py, test_cli.py, test_plot.py)
data/                      # existing
```

**Structure Decision**: workspace with `crates/core` + `crates/py` and a mixed maturin package.
`src/` (migrated to `crates/core`), `main.py` and the `polars`/`anyhow` dependencies are removed.

## Complexity Tracking

| Deviation | Why needed | Simpler alternative rejected because |
|---|---|---|
| Base dependency `polars` (constitution, Technical constraints) replaced by `csv` | Only one numeric column is read; `polars` adds tens of MB to the wheel and minutes of compile time (R13) | Keeping `polars` is not simpler for this use and hurts the "one `pip install`" goal; a constitution PATCH should update the base list |
