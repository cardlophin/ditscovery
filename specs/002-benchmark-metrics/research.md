# Research: Ground-truth metrics, benchmark and dashboard reports

## R1. PROM (precision-recall under optimal matching)

Source: Van Wesenbeeck et al., "Quantitative evaluation of motif sets in time series", DAMI 2025
(arXiv 2412.09346); reference code `ML-KULeuven/tsmd-evaluation` (MIT).

- **Decision**: implement PROM in `crates/core/src/metrics.rs`:
  1. Overlap rate `OR(a, b) = |a ∩ b| / |a ∪ b|` (existing `synth::iou`, moved to `metrics`).
  2. For every GT occurrence β, take the discovered occurrence α (over all discovered sets) with
     the highest OR; match if `OR > 0.5` (strict). Ties: lowest (set, index). Each α matches at
     most one β because GT occurrences do not overlap (OR > 0.5 with two disjoint segments is
     impossible).
  3. Contingency `M ∈ ℕ^{g×d}`: `M[i][j]` = GT occurrences of set i matched to set j; plus an
     "unmatched" column (missed GT occurrences) and an "unmatched" row (discovered occurrences
     matched to nothing).
  4. Optimal assignment of GT sets to discovered sets maximising the diagonal (Hungarian).
  5. `TP = Σ diag`, `FN_i` = rest of row i, `FP_j` = rest of column j. Unassigned discovered sets:
     `FP = 0` with `off_target = ignore` (default), counted with `penalize`.
  6. Micro (default): pooled TP/FP/FN. Macro: mean of per-set P/R/F1 over the m assigned pairs.
  7. Undefined ratios (0/0) are 0.
- **Hungarian**: own O(n³) implementation on the square-padded matrix (~35 lines); `g, d ≤ ~20`.
  No new dependency (Constitution III). Verified against brute force over permutations with
  proptest for `g, d ≤ 6`.
- **Spike (T002) outcome**: the reference (`tsmd_evaluation.prom.matching_matrix`) uses
  `scipy.optimize.linear_sum_assignment` and, when `g < d`, re-assigns tied rows to the discovered
  set with fewest unmatched occurrences (`break_ties`). Which optimal assignment is chosen changes
  precision with `ignore` and the macro values, so to meet SC-001 the core ports scipy's
  shortest-augmenting-path LSAP (Crouse 2016: reversed `remaining` scan, ties prefer unassigned
  columns, transpose when rows > columns) and the same tie-break step. Reference formulas kept
  as is: micro P (ignore) = TP / Σ assigned columns; macro P = Σ p_j / (d | m), macro R = Σ r_i / g,
  macro F1 = Σ f_ij / (d | m) when d ≥ g, else / g. 0/0 and "no discovered sets" ⇒ 0 (the
  reference returns NaN for P there).
- **Oracle test**: pytest compares our PROM with `tsmd-evaluation` on ≥ 1 000 random cases
  (SC-001) when the `bench` extra is installed.
- **Alternatives**: `pathfinding::kuhn_munkres` (extra transitive deps for 35 lines); greedy set
  assignment (not optimal, diverges from the reference).

## R2. Baseline adapters

Common signature (Python): `fn(x: np.ndarray, min_len, max_len, k, seed, **params) -> list[list[(start, len)]]`.

- **LoCoMotif** (`pip install dtai-locomotif`, MIT): `locomotif.apply_locomotif(ts, l_min,
  l_max, rho, nb=k, overlap, warping)`; input z-normalised 2-D `(n, 1)`; output
  `[(candidate, motif_set)]` with `(start, end)` pairs → `(start, end − start)`. Defaults:
  `rho = 0.8`, `warping = True`, `overlap = 0.0`. Spike (dtai-locomotif, 2026-10): signature
  `apply_locomotif(ts, l_min, l_max, rho, nb, start_mask, end_mask, overlap, warping)`; segments
  are `(b, e)` with `e` exclusive (`path[-1][0] + 1`).
- STOMP spike: first `stumpy.stump` call ≈ 15 s of numba JIT (warm-up is mandatory);
  `stumpy.motifs` may return fewer than `max_motifs` sets. Its default exclusion zone (m/4)
  lets matches of one set overlap (trivial matches 30 points apart for m = 60), which GT never
  does; the adapter sets `STUMPY_EXCL_ZONE_DENOM = 1` (zone = m).
- **Finding (implementation)**: LoCoMotif scores 0 on the synthetic suite but works on
  TSMD-Bench (ECG5000 F1 up to 0.89). Its similarity is point-wise on the globally z-normalised
  values, while `synth::planted` writes each occurrence as `level + shape` on a random walk, so
  occurrences sit at different offsets; ditscover and STOMP z-normalise each window and are
  offset-invariant. The synthetic suite therefore favours offset-invariant methods; compare
  methods on TSMD-Bench, and read synthetic results as "variable length + offset" behaviour.
- **Matrix Profile (STOMP)** (`stumpy`, BSD-3): for each `m` in `lengths` (default
  `[round((min_len+max_len)/2)]`): `P = stumpy.stump(x, m)`, then
  `stumpy.motifs(x, P[:, 0], max_motifs=k, max_matches=max_occ)` → each row of indices is a set
  of length-`m` occurrences. With several `m`, pool all sets, rank by mean match distance / √m and
  keep the best `k` whose coverage does not overlap a set already kept.
- **ditscover**: in-process `ds.discover(x, top_k=k, min_len, max_len, seed, **params)`.
- Missing package ⇒ `ImportError` re-raised as `RuntimeError("install ditscover[bench]")` during
  validation, before any run.
- Versions of every method are stored in the effective benchmark configuration.

## R3. Isolation and cost measurement

- **Decision**: one subprocess per run: `python -m ditscover.bench.worker` reads a job (JSON on
  stdin), writes the result (JSON on stdout). Parent uses `subprocess.run(timeout=timeout_s)`;
  timeout / non-zero exit / invalid JSON ⇒ `status = timeout | error` with the stderr tail.
- **Time**: measured inside the worker with `perf_counter` around the method call only.
  JIT warm-up (numba in stumpy / locomotif) is excluded by calling the method once on a 4·max_len
  synthetic series before timing.
- **Memory**: `resource.getrusage(RUSAGE_SELF).ru_maxrss` after warm-up and after the run;
  reported `peak_mb` = peak after run, `delta_mb` = increase over warm-up peak (KB on Linux,
  bytes on macOS). Windows: `null`. `delta_mb` is the figure compared across methods (summary,
  Cost tab, scaling curves) because it excludes import / JIT overhead that differs per library;
  `peak_mb` is kept per run. `ru_maxrss` is a high-water mark, so `delta_mb` is 0 when the run
  never exceeds the warm-up peak; the warm-up series (4·max_len) keeps that baseline small.
- **Threads**: `threads` option sets `RAYON_NUM_THREADS`, `NUMBA_NUM_THREADS`,
  `OMP_NUM_THREADS` in the worker env; default all cores; recorded per run.
- Runs are sequential by default (clean timings); `jobs > 1` allowed for quality-only sweeps and
  recorded in the summary.
- **Alternatives**: in-process runs (no isolation of crashes / memory); `psutil` sampling (extra
  dependency, sampling misses peaks).

## R4. Benchmark configuration

- **Decision**: `BenchConfig` in `crates/core/src/bench.rs` (serde, `deny_unknown_fields`,
  `validate()`), exposed through `_core.build_bench_config(path, overrides)` (Constitution VIII:
  one validated config, parsed in Rust, no `pyyaml`). Method `params` are a free map; ditscover
  params are validated with `build_config`, baseline params against the adapter signature.
- **Resume key**: `(method, dataset, series, seed, cfg_hash)`; `cfg_hash` = SHA-256 prefix of the
  canonical JSON of (dataset spec, method spec, k, threads), computed with `hashlib` in the
  runner. Rows already in `runs.csv` with that key are skipped.

## R5. Datasets

- **Synthetic suite**: extend `synth::planted` usage with a suite generator (`n_series`, `n`,
  `lens`, `occ`, `stretch`, `noise`, `seed`) writing GT JSON files (variable-length occurrences).
- **TSMD-Bench univariate** (ECG5000, Fungi, Plane, Mallat, Symbols): generated from the UCR
  archive with the reference procedure (`generating_the_benchmark.ipynb` in
  `tsmd-evaluation`): class instances concatenated, same class = one GT set, non-repeating
  instances between motifs, validation / test splits.
  - **Decision (spike T002)**: port the notebook procedure to `python/ditscover/bench/datasets.py`
    with numpy only. The reference `generate_tsmd_benchmark_ts` is importable but breaks on
    pandas 3 (`groupby().apply` drops the label column ⇒ `KeyError`), and a git dependency cannot
    go in published wheel metadata. Procedure (notebook): seed 0; train + test instances
    combined and z-normalised per instance; stratified 20 % ⇒ `validation`, rest ⇒ `test`;
    `g ~ U{1, ⌊(C+1)/3⌋}` per series; two instances per repeating class plus extra ones until
    there is one more motif than non-repeating instances; one instance of every other class;
    concatenation alternates motif / non-motif. 250 series per dataset (50 validation, 200 test).
    Data: `https://www.timeseriesclassification.com/aeon-toolkit/<Name>.zip` (`_TRAIN.txt`,
    `_TEST.txt`: label then values), downloaded once into the cache.
  - **Length range (implementation finding)**: `min_len` / `max_len` of every labelled dataset =
    GT occurrence lengths of the dataset widened by ±20 % (`BAND`). TSMD classes are fixed-length;
    with `l_min == l_max` LoCoMotif returns no motif set at all, and the GA gets no stretch room.
    Overridable per method in `params`.
  - **numba cache**: workers get `NUMBA_CACHE_DIR=<cache>/numba` and the STOMP adapter enables
    on-disk caching of stumpy's njit functions: per-process start-up 17 s → 1.9 s (SC-002).
  - `tsmd-evaluation` stays a test-only oracle for PROM (`uv pip install
    git+https://github.com/ML-KULeuven/tsmd-evaluation`), not part of the `bench` extra.
  - Cache: `~/.cache/ditscover/tsmd/<name>/<split>/<i>.json` (GT format). Default `n_series = 20`
    of the test split to keep runtimes reasonable; configurable up to 200.

## R6. Aggregation

- Failed runs (`error | timeout`) on labelled datasets count as P = R = F1 = 0 everywhere (means,
  ranks, `best`), so a method cannot win by failing on hard series; `failures` counts them too.
  Time and memory means use successful runs only.
- Per (method, dataset): mean ± std of P, R, F1, time, peak / delta memory, failures.
- Average rank by F1 per series (ties → average rank), averaged per dataset and overall.
  Stochastic methods: F1 averaged over seeds before ranking. `best` = highest mean F1 over all
  labelled runs (US4), ties broken by lowest overall average rank.
- Datasets without GT (yfinance): quality fields null, excluded from ranks and `best`; they
  contribute cost, `n_sets` and failures only.
- Scaling: median time / delta memory over `repeats` per (method, n).

## R7. Dashboard reports

- **Decision**: `python/ditscover/report.py` with one HTML shell used by both reports:
  header with cards, optional sortable table, tab bar, one panel per tab, plotly.js included once
  (inline by default, CDN with `--cdn`).
- Tabs: buttons + panels toggled with `hidden`; on show, `Plotly.Plots.resize` for the panel's
  graphs (charts drawn in hidden panels otherwise collapse to zero width — FR-019).
- Sorting: click on `<th>` sorts rows by the cell's `data-v` numeric value (~15 lines of JS).
- Light / dark: CSS variables under `prefers-color-scheme`; on load, a small script relayouts every
  graph with matching font / grid / background colours.
- Run report: cards (stop, generations, elapsed, final HV, motifs, dropped, P / R / F1 if GT),
  motif table, tabs Motifs / Evolution / Pareto front (existing figures from `plot.py`, unchanged).
- Bench report: cards (best method by mean F1, runs, failures), tabs Summary (table), Quality
  (grouped bars P / R / F1 per dataset with std error bars), Cost (time and memory per method,
  log axis; scaling curves log-log). One fixed colour per method across all charts.
- **Alternatives**: Plotly Dash / Panel (server needed); Jinja2 templates (extra dependency for a
  single template).

## R9. Yahoo Finance source

- **Decision**: `yfinance` (Apache-2.0, 1.7.0 as of 2026-08) in a new optional extra `finance`.
  Call: `yf.download(ticker, start, end, period, interval, auto_adjust=True, progress=False,
  multi_level_index=False)`; take `field` (default `Close`, adjusted because `auto_adjust=True`).
  Valid intervals: 1m … 1h, 1d, 5d, 1wk, 1mo, 3mo; intraday limited to recent history.
- **Where**: the Rust core cannot use a Python library, so the source is *resolved in Python*
  (`python/ditscover/sources.py`) before calling the core: fetch → values + ISO timestamps →
  `_core.discover(values, cfg, timestamps)`. The configuration (fields, validation, defaults) still
  lives in Rust (Constitution VIII); the core rejects a market source when no values are passed,
  with an error pointing to the Python package. Recorded in Complexity Tracking.
- **Config**: `data.ticker` (null = disabled), `data.start`, `data.end`, `data.period`,
  `data.interval` (`1d`), `data.field` (`Close`), `data.refresh` (false), `data.time_column`
  (null; CSV date column). Always present with defaults so leaf overrides work
  (`--set ticker=AAPL`). Validation: `ticker` and `path` mutually exclusive; `period` xor
  `start`; `interval` in the valid list.
- **Cache**: `~/.cache/ditscover/yfinance/<sha256(ticker, start, resolved end | period, interval)>.csv`
  with `timestamp,value`; an open-ended request is resolved to today's date at first download and
  that resolved end is written into the effective config, so later runs are reproducible.
- **Timestamps**: `RunResult.timestamps: Option<Vec<String>>` (`#[serde(default)]`), saved as a
  first column of `series.csv`; the CSV loader optionally reads `time_column` as strings.
  Plots use them as the x axis (gaps in trading days are not interpolated: the axis is
  categorical-by-observation with date labels, so motifs keep their shape).
- **Tests**: `yf.download` monkeypatched with a recorded DataFrame; an optional `network`
  marker test is skipped by default.
- **Benchmark**: `DatasetSpec::Yfinance { name, tickers, start, end, period, interval, field }`;
  `min_len` / `max_len` required in that dataset's spec (no GT to infer them); quality metrics
  null; quality ranks ignore these datasets.
- **Alternatives**: Rust HTTP client against Yahoo's unofficial endpoints (fragile, re-implements
  yfinance); `pandas-datareader` (Yahoo backend unmaintained).

## R8. Dependencies

- Rust: none new.
- Python extra `bench`: `stumpy`, `dtai-locomotif`, and `tsmd-evaluation` from git if R5 keeps
  the reference generator. Extra `finance`: `yfinance` (brings `pandas`). Core install unchanged
  (`numpy`, `plotly`).
