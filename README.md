<div align="center">

# ditscover

### Discover repeating patterns. Let their duration vary.

Unsupervised discovery of variable-length motifs in univariate time series.<br>
A pure Rust engine, a Python API, and interactive Plotly reports.

<p>
  <code>Rust</code> ·
  <code>Python</code> ·
  <code>NSGA-II</code> ·
  <code>Variable-length motifs</code> ·
  <code>Offline reports</code>
</p>

[Quick start](#quick-start) ·
[Examples](#example-analyses) ·
[How it works](#how-it-works) ·
[Configuration](#configuration) ·
[Development](#development) ·
[Project status](#project-status)

</div>

---

A motif is a pattern that repeats along a time series. In real data, repetitions
rarely have exactly the same duration: one cycle may last 11 months and the next
13.

`ditscover` searches for these patterns without labels or a fixed window length,
then exposes the trade-offs between occurrence count, similarity, and length.

| Discover | Explore | Inspect | Reproduce |
| :--- | :--- | :--- | :--- |
| Find repeated patterns with variable-duration occurrences | Compare solutions on the Pareto front | Review distinct motifs in interactive reports | Save configuration, input data, and optimization history |

> [!WARNING]
> Planted-motif recovery remains an open problem. The current objectives can
> favor motifs padded with background windows over cleaner patterns.
>
> Reference synthetic tests report median recall of `0.54` and precision of
> `0.19`, below the targets of `0.9` and `0.8`. Inspect the Pareto front rather
> than relying only on the selected motifs.
>
> See [Project status](#project-status) for measurements and limitations.

## Quick start

### 1. Install

Requirements: Rust `1.85+` with edition 2024 support, Python `3.12+`, and
[uv](https://docs.astral.sh/uv/).

From the repository root:

```bash
uv venv
uv pip install -e '.[dev]'
uv run maturin develop --release
```

This installs the `ditscover` Python package and command-line entry point.

### 2. Discover motifs

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

### 3. Explore and save

| Task | API |
| :--- | :--- |
| Inspect selected distinct motifs | `res.motifs` |
| Explore the Pareto front | `res.front` |
| Generate an interactive report | `res.plot()` |
| Save a reproducible run | `res.save(directory)` |
| Restore a saved run | `ds.load(directory)` |

Inputs can be CSV/TSV paths, NumPy arrays, lists, or pandas Series.

Configuration fields can also be passed as keyword arguments using their YAML
names, such as `population=300` or `kind="dtw"`.

> [!IMPORTANT]
> `min_len` and `max_len` are measured on the preprocessed series.
> Occurrence positions are always reported in original indices.

### Prefer the CLI?

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

<details>
<summary>CLI precedence and exit codes</summary>

Configuration precedence, from lowest to highest:

```text
defaults → YAML → --set / shortcuts
```

| Exit code | Meaning |
| :---: | :--- |
| `0` | Success |
| `1` | Data or I/O error |
| `2` | Invalid configuration |
| `130` | Cancelled; the partial result is still written |

</details>

## Example analyses

Both examples use configurations from [`configs/`](configs). Preprocessing
interpolates missing values, removes a linear trend, and applies z-score
normalization. Motif lengths range from 9 to 15 months.

| Dataset | Frequency | Period | Points | Distance | Seed |
| :--- | :--- | :--- | ---: | :--- | ---: |
| US electric production | Monthly | 1985–2018 | 397 | DTW | 42 |
| Australian beer production | Monthly | 1956–1995 | 476 | Z-normalized Euclidean | 69 |

### US electric production

The discovered motifs capture the yearly cycle with winter and summer peaks.

| Top motif | Mean duration | Stop condition | Runtime |
| :--- | :--- | :--- | :--- |
| 17 occurrences | About 14 months | Hypervolume stagnation after 45 generations | About 0.1 s |

[View configuration](configs/electric-production.yaml) ·
[Open interactive report](results/electric-production/report.html)

![Top-3 motifs on the electric production series](docs/images/electric-motifs.png)

<details>
<summary>Inspect aligned occurrences and prototypes</summary>

![Aligned occurrences and prototypes for electric production](docs/images/electric-overlay.png)

</details>

### Australian beer production

The top motif captures an annual rise to a peak followed by a decline.
A second motif family includes a secondary bump.

| Motif family | Occurrences | Mean duration | Pattern |
| :--- | ---: | :--- | :--- |
| First | 22 | About 13 months | Annual rise, peak, and decline |
| Second | 11 | About 15 months | Includes a secondary bump |

[View configuration](configs/monthly-beer-production-in-austr.yaml) ·
[Open interactive report](results/monthly-beer-production-in-austr/report.html)

![Top-3 motifs on the beer production series](docs/images/beer-motifs.png)

<details>
<summary>Inspect aligned occurrences and prototypes</summary>

![Aligned occurrences and prototypes for beer production](docs/images/beer-overlay.png)

</details>

## How it works

### Capabilities

| Capability | Behavior |
| :--- | :--- |
| Unsupervised discovery | No labels or training data are required |
| Variable duration | Occurrences can differ in length within `max_stretch` |
| Multi-objective optimization | Support, cohesion, and length are optimized without objective weights |
| Distinct patterns | Overlap-based clearing preserves multiple motif families |
| Reproducibility | Identical seed and configuration produce identical results on 1 or N threads |
| Python and CLI | Notebook exploration and YAML-based batch execution |
| Offline reports | Self-contained HTML reports generated with Plotly |

### Three objectives, one Pareto front

Each candidate motif is a list of `k` non-overlapping segments represented as
`(start, length)`.

NSGA-II evaluates candidates against three objectives:

| Objective | Definition | Direction |
| :--- | :--- | :---: |
| Support | Number of occurrences, `k` | Maximize ↑ |
| Cohesion | Mean distance of occurrences to their prototype after resampling to a common length | Minimize ↓ |
| Length | Mean occurrence length | Maximize ↑ |

These objectives produce a Pareto front rather than a single weighted score.
Users can inspect the available trade-offs and select patterns that fit their
analysis.

### Search process

1. Initialize candidates randomly, or from a random seed window and its best
   matches found by scanning the series.
2. Apply donation crossover: a child absorbs compatible occurrences from the
   other parent.
3. Apply shift, stretch, add, and remove mutations.
4. Repair candidates to preserve validity.
5. Select survivors with NSGA-II and occurrence-overlap clearing based on
   Jaccard similarity.

| Distance | Description |
| :--- | :--- |
| `zeuclid` | Z-normalized Euclidean; default |
| `euclid` | Raw Euclidean |
| `dtw` | Dynamic time warping with a Sakoe–Chiba band |

Runs stop at the generation limit, time budget, hypervolume stagnation, or
Ctrl+C. The best result found so far is returned.

### Architecture

![ditscover components and discovery flow](docs/images/architecture.png)

```text
 Python API / CLI
        │
        ▼
   PyO3 binding
        │
        ▼
    discover()
        │
        ▼
  Preprocessing
        │
        ▼
  NSGA-II engine
        │
        ▼
     Results
        │
        ├──► Saved run
        └──► Interactive report
```

The engine coordinates variation, evaluation, and survival. A separate logger
thread receives non-blocking events and writes the optimization history to
`history.jsonl`.

[Explore the interactive architecture diagram](.archify/architecture-ditscover-overview-20261003-180638/ditscover-overview.html)

The diagram was generated with Archify, links components to their source lines,
and can be opened in a browser.

## Configuration

Every option and its default is documented in
[`config.example.yaml`](specs/001-ga-motif-discovery/contracts/config.example.yaml).

YAML, CLI flags, and Python keyword arguments share consistent configuration
field names.

| Group | Main fields |
| :--- | :--- |
| `data` | `path`, `column` (name or index), `delimiter`, `header` |
| `preprocess` | Ordered list of `nan`, `detrend`, `smooth`, `normalize`, and `paa` steps |
| `motif` | `min_len`, `max_len`, `max_stretch`, `min_occ`, `max_occ`, `min_gap`, `min_std` |
| `distance` | `kind` (`zeuclid`, `euclid`, or `dtw`), `band` |
| `ga` | `population`, operator probabilities, `scan_step`, `init_greedy`, `niche_jaccard` |
| `stop` | `max_generations`, `time_budget_s`, `stagnation_window`, `stagnation_delta` |
| `output` | `dir`, `top_k`, `dedupe_jaccard`, `snapshot_every` |
| `log` | `level`, `log_every`, `history`, `channel_capacity` |

## Saved runs and reports

Each saved run is self-contained:

```text
results/<run>/
├── results.json      # Motifs, objectives, Pareto front, and stop reason
├── history.jsonl     # Optimization history
├── config.yaml       # Effective configuration and seed
├── series.csv        # Original input series
└── report.html       # Optional interactive report
```

| File | Contents |
| :--- | :--- |
| `results.json` | Top-K motifs, occurrences, prototypes, aligned occurrences, objectives, Pareto front, and stop reason |
| `history.jsonl` | Per-generation hypervolume, front size, best and mean objectives, and front snapshots |
| `config.yaml` | Effective configuration, including the seed |
| `series.csv` | Original series, so plots do not require the source data |
| `report.html` | Interactive views of motifs, aligned occurrences, evolution, and the Pareto front by generation |

## Project status

Reference measurements were collected on Apple M4.

| Check | Measured result | Target | Status |
| :--- | :--- | :--- | :---: |
| Full run: 10,000 points, 200 individuals, 300 generations | 0.87 s | < 30 s | ✅ Met |
| Logging overhead | +0.3% | < 5% | ✅ Met |
| Same seed, 1 vs N threads | Identical results | Identical results | ✅ Met |
| Thread scaling, 8 vs 1 | 2.9× | ≥ 5× | ⚠️ Not met |
| Planted-motif recovery (SC-001) | Median recall 0.54; precision 0.19 | Recall 0.9; precision 0.8 | ⚠️ Not met |

### Current limitation: motif recovery

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

## Development

### Design principles

The codebase separates algorithm logic from bindings and presentation:

- All algorithm code lives in `crates/core` and can be tested without Python.
- PyO3 bindings and the Python package convert types and delegate execution.
- YAML, CLI flags, and Python kwargs share one serializable `Config` with
  consistent field names.
- Logging uses a bounded channel and never blocks the genetic algorithm.
  Events dropped when the channel is full are counted.

These rules are defined in the
[project constitution](.specify/memory/constitution.md).

### Checks

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

<details>
<summary>Repository layout</summary>

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

</details>

<details>
<summary>Synthetic recovery evaluation</summary>

Evaluate synthetic recovery across 10 seeds:

```bash
cargo test -p ditscover-core --release planted -- --nocapture
```

The test plants 3 motifs with 8 occurrences each in a 10,000-point series and
reports recall and precision.

See [Project status](#project-status) for the current reference results.

</details>

<details>
<summary>Benchmarks and regression checks</summary>

Create a reference baseline:

```bash
cargo bench -p ditscover-core --bench ga -- --save-baseline ref
```

Compare subsequent runs and check for regressions above 10%:

```bash
cargo bench -p ditscover-core --bench ga -- --baseline ref
python scripts/bench_check.py
```

</details>

### Specifications

Development follows Spec Kit. The specification, plan, research decisions,
data model,
