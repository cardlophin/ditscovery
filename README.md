# ditscover

Unsupervised discovery of variable-length motifs in univariate time series using
NSGA-II, with a pure Rust engine, a Python API, and interactive Plotly reports.

A *motif* is a pattern that repeats along a series. In real data, repetitions rarely
have exactly the same duration: one cycle may last 11 months and the next 13.
`ditscover` searches for these patterns without labels or a fixed window length.

The repository covers the full workflow:

1. Discover repeated patterns with variable-duration occurrences.
2. Explore trade-offs between occurrence count, similarity, and length.
3. Inspect distinct motifs and the Pareto front through interactive reports.
4. Save reproducible runs with their configuration, input series, and optimization
   history.

> [!WARNING]
> Planted-motif recovery remains an open problem. The current objectives can favor
> motifs padded with background windows over cleaner patterns.
>
> Reference synthetic tests report median recall of 0.54 and precision of 0.19,
> below the targets of 0.9 and 0.8. Inspect the Pareto front rather than relying
> only on the selected motifs. See [Project status](#project-status).

## Overview

![ditscover components and discovery flow](docs/images/architecture.png)

The discovery pipeline follows this path:

```text
Python API / CLI
       ↓
   PyO3 binding
       ↓
   discover()
       ↓
 Preprocessing
       ↓
 NSGA-II engine
       ↓
    Results
       ↓
 Saved run / Interactive report
```

The engine coordinates variation, evaluation, and survival. A separate logger
thread receives non-blocking events and writes the optimization history to
`history.jsonl`.

An [interactive architecture diagram](.archify/architecture-ditscover-overview-20261003-180638/ditscover-overview.html)
links components to their source lines. It was generated with Archify and can be
opened in a browser.

**Capabilities**

| Capability | Behavior |
|---|---|
| Unsupervised discovery | No labels or training data are required |
| Variable duration | Occurrences can differ in length within `max_stretch` |
| Multi-objective optimization | Support, cohesion, and length are optimized without objective weights |
| Distinct patterns | Overlap-based clearing preserves multiple motif families |
| Reproducibility | Identical seed and configuration produce identical results on 1 or N threads |
| Python and CLI | Notebook exploration and YAML-based batch execution |
| Offline reports | Self-contained HTML reports generated with Plotly |

**Objectives**

Each candidate motif is a list of `k` non-overlapping segments represented as
`(start, length)`. NSGA-II evaluates candidates against three objectives:

| Objective | Definition | Direction |
|---|---|---|
| Support | Number of occurrences, `k` | Maximize |
| Cohesion | Mean distance of occurrences to their prototype after resampling to a common length | Minimize |
| Length | Mean occurrence length | Maximize |

These objectives produce a Pareto front rather than a single weighted score.
Users can inspect the available trade-offs and select patterns that fit their
analysis.

**Search**

1. Initialize candidates randomly or from a random seed window and its best
   matches found by scanning the series.
2. Apply donation crossover: a child absorbs compatible occurrences from the
   other parent.
3. Apply shift, stretch, add, and remove mutations.
4. Repair candidates to preserve validity.
5. Select survivors with NSGA-II and occurrence-overlap clearing based on Jaccard
   similarity.

Available distances are z-normalized Euclidean (default), raw Euclidean, and DTW
with a Sakoe–Chiba band.

Runs stop at the generation limit, time budget, hypervolume stagnation, or Ctrl+C.
The best result found so far is returned.

## Installation and usage

Requirements:

- Rust 1.85 or later, with edition 2024 support
- Python 3.12 or later
- [uv](https://docs.astral.sh/uv/)

From the repository root:

```bash
uv venv
uv pip install -e '.[dev]'
uv run maturin develop --release
```

This installs the `ditscover` Python package and command-line entry point.

**Python**

```python
import ditscover as ds

res = ds.discover(
    "data/kaggle-electric-production.csv",
    column="IPG2211A2N",
    min_len=9,
    max_len=15,
    preprocess=[{"detrend": "linear"}],
    seed=1,
)

# Generate a self-contained HTML report that works offline.
res.plot()

# Inspect the selected distinct motifs.
for motif in res.motifs:
    print(
        motif.support,
        motif.cohesion,
        motif.mean_len,
        motif.occurrences[:3],
    )
```

Inputs can be CSV/TSV paths, NumPy arrays, lists, or pandas Series.

Configuration fields can be passed as keyword arguments using their YAML names,
such as `population=300` or `kind="dtw"`.

Save and restore runs with `res.save(directory)` and `ds.load(directory)`.
Selected motifs are available through `res.motifs`; the Pareto front is available
through `res.front`.

> [!IMPORTANT]
> `min_len` and `max_len` are measured on the preprocessed series. Occurrence
> positions are always reported in original indices.

**CLI**

```bash
# Run a configuration and write results to output.dir.
ditscover run configs/electric-production.yaml

# Override settings and generate a report.
ditscover run configs/electric-production.yaml \
  --seed 7 \
  --set population=300 \
  --plot

# Generate a report from a saved run.
ditscover plot results/electric-production
```

Configuration precedence:

```text
defaults < YAML < --set / shortcuts
```

| Exit code | Meaning |
|---|---|
| `0` | Success |
| `1` | Data or I/O error |
| `2` | Invalid configuration |
| `130` | Cancelled; the partial result is still written |

**Configuration**

Every option and its default is documented in
[`config.example.yaml`](specs/001-ga-motif-discovery/contracts/config.example.yaml).

| Group | Main fields |
|---|---|
| `data` | `path`, `column` (name or index), `delimiter`, `header` |
| `preprocess` | Ordered list of `nan`, `detrend`, `smooth`, `normalize`, and `paa` steps |
| `motif` | `min_len`, `max_len`, `max_stretch`, `min_occ`, `max_occ`, `min_gap`, `min_std` |
| `distance` | `kind` (`zeuclid`, `euclid`, or `dtw`), `band` |
| `ga` | `population`, operator probabilities, `scan_step`, `init_greedy`, `niche_jaccard` |
| `stop` | `max_generations`, `time_budget_s`, `stagnation_window`, `stagnation_delta` |
| `output` | `dir`, `top_k`, `dedupe_jaccard`, `snapshot_every` |
| `log` | `level`, `log_every`, `history`, `channel_capacity` |

**Outputs**

Each saved run is self-contained:

```text
results/<run>/
├── results.json
├── history.jsonl
├── config.yaml
├── series.csv
└── report.html       # Optional
```

| File | Contents |
|---|---|
| `results.json` | Top-K motifs, occurrences, prototypes, aligned occurrences, objectives, Pareto front, and stop reason |
| `history.jsonl` | Per-generation hypervolume, front size, best and mean objectives, and front snapshots |
| `config.yaml` | Effective configuration, including the seed |
| `series.csv` | Original series, so plots do not require the source data |
| `report.html` | Interactive views of motifs, aligned occurrences, evolution, and the Pareto front by generation |

## Example analyses

Both examples use configurations from [`configs/`](configs). Preprocessing
interpolates missing values, removes a linear trend, and applies z-score
normalization. Motif lengths range from 9 to 15 months.

| Dataset | Frequency | Period | Points | Distance | Seed |
|---|---|---|---:|---|---:|
| US electric production | Monthly | 1985–2018 | 397 | DTW | 42 |
| Australian beer production | Monthly | 1956–1995 | 476 | Z-normalized Euclidean | 69 |

**Electricity**

The discovered motifs capture the yearly cycle with winter and summer peaks.
The top motif contains 17 occurrences of about 14 months.

The run stopped on hypervolume stagnation after 45 generations, taking about
0.1 s.

[Configuration](configs/electric-production.yaml) ·
[Interactive report](results/electric-production/report.html)

![Top-3 motifs on the electric production series](docs/images/electric-motifs.png)

![Aligned occurrences and prototypes for electric production](docs/images/electric-overlay.png)

**Beer**

The top motif contains 22 occurrences of about 13 months. Its prototype captures
an annual rise to a peak followed by a decline.

A second family contains 11 occurrences of about 15 months and includes a
secondary bump.

[Configuration](configs/monthly-beer-production-in-austr.yaml) ·
[Interactive report](results/monthly-beer-production-in-austr/report.html)

![Top-3 motifs on the beer production series](docs/images/beer-motifs.png)

![Aligned occurrences and prototypes for beer production](docs/images/beer-overlay.png)

## Development

The codebase separates algorithm logic from bindings and presentation:

- All algorithm code lives in `crates/core` and can be tested without Python.
- PyO3 bindings and the Python package convert types and delegate execution.
- YAML, CLI flags, and Python kwargs share one serializable `Config` with
  consistent field names.
- Logging uses a bounded channel and never blocks the genetic algorithm.
  Events dropped when the channel is full are counted.

These rules are defined in the
[project constitution](.specify/memory/constitution.md).

**Layout**

```text
.
├── Cargo.toml                   # Rust workspace
├── pyproject.toml               # maturin, ruff, pytest
├── crates/
│   ├── core/                    # Pure Rust algorithm
│   │   ├── src/
│   │   │   ├── lib.rs           # Discovery pipeline
│   │   │   ├── config.rs        # Defaults, validation, overrides
│   │   │   ├── io.rs            # Loader trait and CSV loader
│   │   │   ├── preprocess.rs    # NaN, detrend, smooth, normalize, PAA
│   │   │   ├── series.rs        # Series, origin map, prefix statistics
│   │   │   ├── distance.rs      # Resampling, Euclidean, DTW
│   │   │   ├── motif.rs         # Segments, evaluation, prototypes, Jaccard
│   │   │   ├── ops.rs           # Initialization, crossover, mutations, repair
│   │   │   ├── nsga.rs          # Sorting, crowding, clearing, hypervolume
│   │   │   ├── engine.rs        # Evolution loop, RNG, stopping
│   │   │   ├── events.rs        # Event channel and logger
│   │   │   ├── result.rs        # Top-K selection, save/load
│   │   │   └── synth.rs         # Planted motifs and IoU metrics
│   │   ├── tests/               # Integration and property tests
│   │   └── benches/ga.rs        # Criterion benchmarks
│   └── py/                      # PyO3 bindings: ditscover._core
├── python/ditscover/             # Python API, CLI, Plotly views
├── tests/python/                # API, CLI, and plot tests
├── configs/                     # Example configurations
├── data/                        # Example datasets
├── results/                     # Saved runs
├── docs/images/                 # README figures
├── scripts/bench_check.py        # Benchmark regression check
├── specs/001-ga-motif-discovery/ # Specification and implementation plan
└── .archify/                    # Interactive architecture diagram
```

**Checks**

```bash
# Rust formatting, linting, and tests.
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test

# Python linting, formatting, and tests.
uv run ruff check
uv run ruff format --check
uv run pytest tests/python
```

The test suite covers:

- Operator invariants through property tests with proptest.
- Determinism across 1 and 4 threads.
- Stop criteria and cancellation.
- Save/load round trips.
- Allocation-free evaluation in the hot loop.
- Python API, CLI, and plotting behavior.

**Recovery**

Evaluate synthetic recovery across 10 seeds:

```bash
cargo test -p ditscover-core --release planted -- --nocapture
```

The test plants 3 motifs with 8 occurrences each in a 10,000-point series and
reports recall and precision.

**Benchmarks**

Create a reference baseline:

```bash
cargo bench -p ditscover-core --bench ga -- --save-baseline ref
```

Compare subsequent runs and check for regressions above 10%:

```bash
cargo bench -p ditscover-core --bench ga -- --baseline ref
python scripts/bench_check.py
```

Development follows Spec Kit. The specification, plan, research decisions,
data model, contracts, and tasks are available in
[`specs/001-ga-motif-discovery/`](specs/001-ga-motif-discovery/).

## Project status

Reference measurements were collected on Apple M4.

| Check | Measured result | Target | Status |
|---|---|---|---|
| Full run: 10,000 points, 200 individuals, 300 generations | 0.87 s | < 30 s | Met |
| Logging overhead | +0.3% | < 5% | Met |
| Same seed, 1 vs N threads | Identical results | Identical results | Met |
| Thread scaling, 8 vs 1 | 2.9× | ≥ 5× | Not met |
| Planted-motif recovery (SC-001) | Median recall 0.54; precision 0.19 | Recall 0.9; precision 0.8 | Not met |

> [!WARNING]
> Maximizing support can reward adding background windows to a real motif up to
> `max_occ`. Top-K selection can therefore prefer padded candidates over cleaner
> patterns.
>
> Resolving this requires a change to the objectives or top-K selection.

For now:

- Set `max_occ` close to the number of repetitions you expect.
- Inspect `res.front`, not only `res.motifs`.
- Review aligned occurrences and prototypes before interpreting a discovered
  motif.
