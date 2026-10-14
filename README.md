<div align="center">

# 🔎 ditscover

### Repeating patterns. Different durations. One discovery engine.

Discover variable-length motifs in time series — without labels or fixed windows.

<br>

<img src="https://img.shields.io/badge/engine-Rust-f97316?style=for-the-badge&logo=rust&logoColor=white" alt="Rust engine">
<img src="https://img.shields.io/badge/API-Python-3b82f6?style=for-the-badge&logo=python&logoColor=white" alt="Python API">
<img src="https://img.shields.io/badge/search-NSGA--II-8b5cf6?style=for-the-badge" alt="NSGA-II search">
<img src="https://img.shields.io/badge/reports-Plotly-06b6d4?style=for-the-badge&logo=plotly&logoColor=white" alt="Plotly reports">

<br>
<br>

<a href="#quick-start">🚀 Quick start</a>
&nbsp;·&nbsp;
<a href="#examples">📊 Examples</a>
&nbsp;·&nbsp;
<a href="#benchmark">🏁 Benchmark</a>
&nbsp;·&nbsp;
<a href="#how-it-works">🧬 How it works</a>
&nbsp;·&nbsp;
<a href="#configuration">⚙️ Configuration</a>
&nbsp;·&nbsp;
<a href="#development">🛠️ Development</a>
&nbsp;·&nbsp;
<a href="#project-status">🚧 Status</a>

<br>
<br>

<img src="docs/images/electric-motifs.png" alt="Discovered motifs on the electric production series" width="100%">

<sub>Variable-length occurrences discovered in the US electric production series.</sub>

</div>

<br>

## ✨ Same pattern. A different tempo.

A motif is a pattern that repeats along a time series. In real data, repetitions
rarely have exactly the same duration: one cycle may last 11 months and the next
13.

`ditscover` searches for these patterns without labels or a fixed window length.
Its pure Rust engine uses NSGA-II to explore trade-offs between occurrence count,
similarity, and length — while Python and interactive reports make the results
easy to inspect.

| 🔎 Discover | 🎛️ Explore | 📊 Inspect | 📦 Reproduce |
| :---: | :---: | :---: | :---: |
| Repeated patterns with variable durations | Trade-offs on the Pareto front | Distinct motifs and aligned occurrences | Configuration, data, and optimization history |

> [!NOTE]
> On labelled benchmarks, `ditscover` is competitive with Matrix Profile (STOMP)
> and LoCoMotif: it has the best mean rank over six datasets and is the best
> method on the variable-length synthetic suite. It is not perfect — see
> [Benchmark](#benchmark) for where it falls short.

---

<a id="quick-start"></a>

## 🚀 From time series to motifs

### ① Install the project

<img src="https://img.shields.io/badge/Rust-1.85%2B-f97316?style=flat-square&logo=rust&logoColor=white" alt="Rust 1.85 or later">
<img src="https://img.shields.io/badge/Python-3.12%2B-3b82f6?style=flat-square&logo=python&logoColor=white" alt="Python 3.12 or later">
<img src="https://img.shields.io/badge/package_manager-uv-8b5cf6?style=flat-square" alt="uv package manager">

Rust must support edition 2024. Install [uv](https://docs.astral.sh/uv/), then run
the following from the repository root:

```bash
uv venv
uv pip install -e '.[dev]'
uv run maturin develop --release
```

This installs the `ditscover` Python package and command-line entry point.

### ② Discover repeating patterns

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

# Generate an interactive HTML report.
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

### ③ Explore, save, and revisit

| What you want to do | Where to look |
| :--- | :--- |
| 🔎 Inspect selected distinct motifs | `res.motifs` |
| 🎛️ Explore alternative trade-offs | `res.front` |
| 📊 Generate an offline HTML report | `res.plot()` |
| 💾 Save a reproducible run | `res.save(directory)` |
| 📂 Restore a previous run | `ds.load(directory)` |

> [!IMPORTANT]
> `min_len` and `max_len` are measured on the preprocessed series.
> Occurrence positions are always reported in original indices.

### 🖥️ Prefer the terminal?

```bash
# Run a configuration and save the results.
ditscover run configs/electric-production.yaml

# Override settings and generate a report.
ditscover run configs/electric-production.yaml \
  --seed 7 \
  --set population=300 \
  --plot

# Generate a report from an existing run.
ditscover plot results/electric-production
```

<details>
<summary>🔧 Configuration precedence and exit codes</summary>

<br>

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

---

<a id="examples"></a>

## 📊 Patterns in the wild

Both examples use configurations from [`configs/`](configs).

Preprocessing interpolates missing values, removes a linear trend, and applies
z-score normalization. Motif lengths range from 9 to 15 months.

| Dataset | Frequency | Period | Points | Distance | Seed |
| :--- | :--- | :--- | ---: | :--- | ---: |
| ⚡ US electric production | Monthly | 1985–2018 | 397 | DTW | 42 |
| 🍺 Australian beer production | Monthly | 1956–1995 | 476 | Z-normalized Euclidean | 69 |

### ⚡ Electricity · a recurring seasonal rhythm

The discovered motifs capture the yearly cycle with winter and summer peaks.

<div align="center">

<img src="https://img.shields.io/badge/occurrences-17-3b82f6?style=for-the-badge" alt="17 occurrences">
<img src="https://img.shields.io/badge/duration-~14_months-8b5cf6?style=for-the-badge" alt="About 14 months">
<img src="https://img.shields.io/badge/generations-45-06b6d4?style=for-the-badge" alt="45 generations">

</div>

<br>

The run stopped on hypervolume stagnation after 45 generations, taking about
0.1 s.

[⚙️ Configuration](configs/electric-production.yaml) ·
[🌐 Interactive report](results/electric-production/report.html)

![Aligned occurrences and prototypes for electric production](docs/images/electric-overlay.png)

<details>
<summary>🔎 View occurrences across the full series</summary>

<br>

![Top-3 motifs on the electric production series](docs/images/electric-motifs.png)

</details>

### 🍺 Beer · an annual rise, peak, and decline

The top motif captures an annual rise to a peak followed by a decline.
A second family includes a secondary bump.

<div align="center">

<img src="https://img.shields.io/badge/top_motif-22_occurrences-ec4899?style=for-the-badge" alt="Top motif with 22 occurrences">
<img src="https://img.shields.io/badge/duration-~13_months-8b5cf6?style=for-the-badge" alt="About 13 months">
<img src="https://img.shields.io/badge/second_family-11_occurrences-06b6d4?style=for-the-badge" alt="Second family with 11 occurrences">

</div>

<br>

| Motif family | Occurrences | Mean duration | Shape |
| :--- | ---: | :--- | :--- |
| First | 22 | About 13 months | Annual rise, peak, and decline |
| Second | 11 | About 15 months | Includes a secondary bump |

[⚙️ Configuration](configs/monthly-beer-production-in-austr.yaml) ·
[🌐 Interactive report](results/monthly-beer-production-in-austr/report.html)

![Top-3 motifs on the beer production series](docs/images/beer-motifs.png)

<details>
<summary>🔎 View aligned occurrences and prototypes</summary>

<br>

![Aligned occurrences and prototypes for beer production](docs/images/beer-overlay.png)

</details>

---

<a id="benchmark"></a>

## 🏁 How it compares

`ditscover` was compared with two state-of-the-art motif discovery methods on
labelled data, where the true motif occurrences are known:

| Method | What it is |
| :--- | :--- |
| **STOMP** | Matrix Profile motifs ([stumpy](https://github.com/TDAmeritrade/stumpy)); fixed window length |
| **LoCoMotif** | Variable-length motif sets ([locomotif](https://github.com/ML-KULeuven/locomotif)); time-warped |
| **ditscover** | This library, with its default configuration |

<details>
<summary>🔬 Protocol</summary>

<br>

- **Data**: a synthetic suite with three planted motifs of different lengths
  (40, 60, 80) stretched by up to 30 %, plus five univariate datasets from
  TSMD-Bench (ECG5000, Fungi, Plane, Mallat, Symbols). 20 unseen test series
  per dataset.
- **Fairness**: every method receives the true number of motif sets. STOMP and
  LoCoMotif hyper-parameters are tuned on a separate validation split;
  `ditscover` uses its library defaults with 3 seeds.
- **Metric**: PROM F1. A found occurrence counts as correct when it overlaps a
  true occurrence of the matched motif set with IoU > 0.5.
- **Statistics**: paired Wilcoxon tests over series, Holm-corrected.

</details>

<br>

![Mean F1 per dataset for STOMP, LoCoMotif and ditscover](docs/images/bench-f1.png)

| Dataset | STOMP | LoCoMotif | ditscover | ditscover vs best baseline |
| :--- | ---: | ---: | ---: | :--- |
| 🧪 Synthetic, variable length | 0.53 ± 0.15 | 0.08 ± 0.07 | **0.61 ± 0.24** | 🟢 Best; well above LoCoMotif |
| 🫀 ECG5000 | 0.54 ± 0.29 | 0.57 ± 0.21 | **0.58 ± 0.20** | 🟢 Best; on par with both |
| ✈️ Plane | 0.74 ± 0.12 | **0.89 ± 0.11** | 0.87 ± 0.16 | 🟢 On par with LoCoMotif; above STOMP |
| ✍️ Symbols | 0.57 ± 0.32 | **0.78 ± 0.18** | 0.77 ± 0.19 | 🟢 On par with LoCoMotif |
| 🌊 Mallat | 0.31 ± 0.28 | **0.64 ± 0.24** | 0.35 ± 0.27 | 🔴 Below LoCoMotif |
| 🍄 Fungi | **0.65 ± 0.22** | 0.05 ± 0.07 | 0.31 ± 0.33 | 🔴 Below STOMP; above LoCoMotif |
| **Mean rank** (lower is better) | 2.08 | 2.03 | **1.89** | |

<sub>Mean F1 ± std on the test split. "On par" means no statistically significant
difference (Holm-corrected p ≥ 0.05).</sub>

`ditscover` never needs a window length, and it is the only method that is
never last on any dataset: STOMP struggles with variable-length motifs, and
LoCoMotif almost never finds the right windows on the synthetic suite or Fungi.

### 🏷️ What the labelling looks like

Each figure shows one test series. Shaded areas are the true occurrences,
colored by motif class. Below, each method's windows are colored by the class
of the true motif set they were matched to:

| Style | Meaning |
| :--- | :--- |
| Solid | ✅ Hit: correct window and class |
| Hatched | ❌ False alarm: no true occurrence of that class there |
| Dotted outline | ⭕ Miss: true occurrence the method did not find |

**🧪 Synthetic: three motifs, each repeated with different durations.**
`ditscover` recovers all three classes almost perfectly. LoCoMotif fragments
them into many short windows.

![Labelling of a synthetic series](docs/images/label-synth.png)

**✍️ Symbols: two hand-drawn shapes.** STOMP's fixed window cannot follow the
shapes; `ditscover` labels four of five occurrences with the right class.

![Labelling of a Symbols series](docs/images/label-symbols.png)

<details>
<summary>🫀 ECG5000 and ✈️ Plane</summary>

<br>

![Labelling of an ECG5000 series](docs/images/label-ecg5000.png)

![Labelling of a Plane series](docs/images/label-plane.png)

</details>

### 🍄 Where it falls short

On **Fungi**, occurrences of the three classes are almost identical spikes
packed back to back. `ditscover` keeps returned occurrences from overlapping,
so it places windows between the true ones and mixes the classes; STOMP does
better here. On **Mallat** (motifs of about 1,000 points), LoCoMotif is
clearly ahead.

![Labelling of a Fungi series, a failure case](docs/images/label-fungi.png)

**Speed.** A run takes 0.5–1.2 s per series (median). That is faster than
LoCoMotif on the synthetic suite and on Mallat (1.1 s vs 5.3 s) but slower on
the short series, where LoCoMotif takes well under a second and STOMP takes a
few milliseconds.

<details>
<summary>🔁 Reproduce the benchmark</summary>

<br>

```bash
uv pip install -e '.[dev,bench]'
ditscover bench specs/003-motif-quality/contracts/final.example.yaml \
  --out results/bench-003 --plot
```

The interactive report (`results/bench-003/report.html`) has an **Errors** tab
with the labelling of every test series for every method. Full numbers and
tests are in
[`research.md`](specs/003-motif-quality/research.md#r9-final-benchmark-test-split).

</details>

---

<a id="how-it-works"></a>

## 🧬 Inside the discovery engine

### What makes it different?

| | Capability | Behavior |
| :---: | :--- | :--- |
| 🏷️ | Unsupervised discovery | No labels or training data are required |
| ↔️ | Variable duration | Occurrences can differ in length within `max_stretch` |
| 🎯 | Multi-objective optimization | Support, cohesion, and length are optimized without objective weights |
| 🧩 | Distinct patterns | Overlap-based clearing preserves multiple motif families |
| 🎲 | Reproducibility | Identical seed and configuration produce identical results on 1 or N threads |
| 🐍 | Python and CLI | Notebook exploration and YAML-based batch execution |
| 🌐 | Offline reports | Self-contained HTML reports generated with Plotly |

### Three objectives. No weighted score.

Each candidate motif is a list of `k` non-overlapping segments represented as
`(start, length)`.

NSGA-II evaluates candidates against three objectives:

| Objective | What it measures | Direction |
| :--- | :--- | :---: |
| 🔵 Support | Number of occurrences, `k` | Maximize ↑ |
| 🟣 Cohesion | Mean distance of occurrences to their prototype after resampling to a common length | Minimize ↓ |
| 🟢 Length | Mean occurrence length | Maximize ↑ |

These objectives produce a Pareto front rather than a single weighted score.
Users can inspect the available trade-offs and select patterns that fit their
analysis.

### The evolutionary loop

```text
   🌱 Initialize
         │
         ▼
   🧬 Crossover
         │
         ▼
   🎲 Mutate
         │
         ▼
   🔧 Repair
         │
         ▼
   🎯 Evaluate
         │
         ▼
   🏆 Select survivors
         │
         └──────────► Repeat until a stop criterion is reached
```

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
| `zeuclid` | Z-normalized Euclidean — default |
| `euclid` | Raw Euclidean |
| `dtw` | Dynamic time warping with a Sakoe–Chiba band |

Runs stop at the generation limit, time budget, hypervolume stagnation, or
Ctrl+C. The best result found so far is returned.

### 🏗️ Architecture at a glance

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
        ├──► 📦 Saved run
        └──► 📊 Interactive report
```

The engine coordinates variation, evaluation, and survival. A separate logger
thread receives non-blocking events and writes the optimization history to
`history.jsonl`.

[🗺️ Explore the interactive architecture diagram](.archify/architecture-ditscover-overview-20261003-180638/ditscover-overview.html)

Generated with Archify, the diagram links components to their source lines and
can be opened in a browser.

---

<a id="configuration"></a>

## ⚙️ Make the search your own

Every option and its default is documented in
[`config.example.yaml`](specs/001-ga-motif-discovery/contracts/config.example.yaml).

YAML, CLI flags, and Python keyword arguments share consistent field names.

| Group | Main fields |
| :--- | :--- |
| 📥 `data` | `path`, `column` (name or index), `delimiter`, `header` |
| 🧹 `preprocess` | Ordered list of `nan`, `detrend`, `smooth`, `normalize`, and `paa` steps |
| 🧩 `motif` | `min_len`, `max_len`, `max_stretch`, `min_occ`, `max_occ`, `min_gap`, `min_std` |
| 📏 `distance` | `kind` (`zeuclid`, `euclid`, or `dtw`), `band` |
| 🧬 `ga` | `population`, operator probabilities, `scan_step`, `init_greedy`, `niche_jaccard` |
| ⏱️ `stop` | `max_generations`, `time_budget_s`, `stagnation_window`, `stagnation_delta` |
| 📦 `output` | `dir`, `top_k`, `dedupe_jaccard`, `snapshot_every` |
| 📝 `log` | `level`, `log_every`, `history`, `channel_capacity` |

## 📦 Every run tells its own story

Each saved run is self-contained:

```text
results/<run>/
│
├── results.json      # 🔎 Motifs, objectives, and Pareto front
├── history.jsonl     # 📈 Optimization history
├── config.yaml       # ⚙️ Effective configuration and seed
├── series.csv        # 📊 Original input series
└── report.html       # 🌐 Optional interactive report
```

<details>
<summary>📂 What each file contains</summary>

<br>

| File | Contents |
| :--- | :--- |
| `results.json` | Top-K motifs, occurrences, prototypes, aligned occurrences, objectives, Pareto front, and stop reason |
| `history.jsonl` | Per-generation hypervolume, front size, best and mean objectives, and front snapshots |
| `config.yaml` | Effective configuration, including the seed |
| `series.csv` | Original series, so plots do not require the source data |
| `report.html` | Interactive views of motifs, aligned occurrences, evolution, and the Pareto front by generation |

</details>

---

<a id="project-status"></a>

## 🚧 Fast engine. Open research questions.

Reference measurements were collected on Apple M4.

| Check | Measured result | Target | Status |
| :--- | :--- | :--- | :---: |
| Full run: 10,000 points, 200 individuals, 300 generations | 0.87 s | < 30 s | 🟢 Met |
| Logging overhead | +0.3% | < 5% | 🟢 Met |
| Same seed, 1 vs N threads | Identical results | Identical results | 🟢 Met |
| Thread scaling, 8 vs 1 | 2.9× | ≥ 5× | 🟠 Not met |
| Planted-motif recovery, 10,000 points (feature 001) | Median recall 0.63; precision 0.63 (was 0.54 / 0.19) | Recall 0.9; precision 0.8 | 🟠 Not met |
| Synthetic suite, test split (feature 003) | Median F1 0.60; precision 0.67 | ≥ 0.70 each | 🟠 Close |
| Not worse than the best baseline on any dataset | Below on Fungi and Mallat | — | 🟠 Not met |
| Defaults on 20,000 points | ~2 s per run | < 10 s | 🟢 Met |
| Returned motifs never overlap | 0 conflicts in 360 runs | 0 | 🟢 Met |

### Remaining limitations

- Closely packed, near-identical motifs (Fungi) and very long motifs (Mallat)
  are still recovered worse than by the best baseline. See
  [Benchmark](#benchmark).
- 🔎 When results look off, inspect `res.front`, not only `res.motifs`, and
  review aligned occurrences and prototypes before interpreting a motif.

---

<a id="development"></a>

## 🛠️ Built to be explored

### Design principles

| Principle | Implementation |
| :--- | :--- |
| 🦀 Rust owns the algorithm | All algorithm code lives in `crates/core` and can be tested without Python |
| 🐍 Bindings stay thin | PyO3 and the Python package convert types and delegate execution |
| ⚙️ One configuration model | YAML, CLI flags, and Python kwargs share one serializable `Config` |
| 📝 Logging stays out of the way | A bounded channel never blocks the genetic algorithm; dropped events are counted |

These rules are defined in the
[project constitution](.specify/memory/constitution.md).

### ✅ Run the checks

```bash
# Rust
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test

# Python
uv run ruff check
uv run ruff format --check
uv run pytest tests/python
```

<details>
<summary>🧪 Test coverage areas</summary>

<br>

- Operator invariants through property tests with proptest.
- Determinism across 1 and 4 threads.
- Stop criteria and cancellation.
- Save/load round trips.
- Allocation-free evaluation in the hot loop.
- Python API, CLI, and plotting behavior.

</details>

<details>
<summary>🗂️ Repository layout</summary>

<br>

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
<summary>🌱 Evaluate synthetic recovery</summary>

<br>

Evaluate recovery across 10 seeds:

```bash
cargo test -p ditscover-core --release planted -- --nocapture
```

The test plants 3 motifs with 8 occurrences each in a 10,000-point series and
reports recall and precision.

See [Project status](#project-status) for current reference measurements.

</details>

<details>
<summary>⏱️ Benchmarks and regression checks</summary>

<br>

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

### 📚 Specifications and research decisions

Development follows Spec Kit. The specification, plan, research decisions,
data model, contracts, and tasks are available in
[`specs/001-ga-motif-discovery/`](specs/001-ga-motif-discovery/).

---

<div align="center">

### 🔎 Find the pattern. Explore the trade-offs.

<sub>Pure Rust at the core · Python at your fingertips · Interactive reports for exploration</sub>

<br>
<br>

[🚀 Get started](#quick-start) ·
[📊 Explore examples](#examples) ·
[🏁 See the benchmark](#benchmark) ·
[🚧 Review limitations](#project-status)
