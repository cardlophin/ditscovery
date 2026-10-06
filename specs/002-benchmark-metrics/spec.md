# Feature Specification: Ground-truth metrics, state-of-the-art benchmark and dashboard reports

**Feature Branch**: `002-benchmark-metrics`

**Created**: 2026-10-05

**Status**: Draft

**Input**: User description (translated): "Next Spec Kit iteration: add metrics and compare our
algorithm against state-of-the-art methods such as Matrix Profile. Also improve the report shown
at the end of a run with `--plot`." Follow-up: "Also allow taking the time series from yfinance to
make it more interesting."

## Clarifications

### Session 2026-10-05

- Q: Which methods are compared? → A: LoCoMotif (state of the art for variable-length motif sets)
  and Matrix Profile (STOMP).
- Q: Where do the baselines live? → A: Python harness using the reference implementations as an
  optional `bench` extra; metrics are computed in the core so every method is measured the same way.
- Q: Which metrics? → A: Ground-truth quality (PROM precision / recall / F1) and cost (time,
  peak memory, scaling with series length).
- Q: Which report improvements? → A: Dashboard with summary cards and a motif table, organised
  in tabs instead of one long page.
- Q: Additional data source? → A: Yahoo Finance market data (via yfinance) for discovery runs and
  as unlabelled benchmark datasets (cost only, no ground truth).
- Q: How does a failed or timed-out run count in quality means and the "best method" card? → A:
  As P = R = F1 = 0 in the means and in the ranks; failures are also counted separately.
- Q: How many motif sets are requested on market-data datasets (no GT)? → A: A fixed number per
  market-data dataset, default 5, overridable in that dataset's configuration.
- Q: Which memory figure is compared across methods? → A: Memory above a baseline taken after
  imports and warm-up; total process peak memory is still recorded per run.

## Glossary

- **Motif set**: a group of occurrences (start, length) a method reports as the same pattern.
- **Ground truth (GT)**: the known motif sets of a series (planted or labelled).
- **Overlap rate**: |a ∩ b| / |a ∪ b| between two occurrences.
- **PROM**: precision-recall under optimal matching (Van Wesenbeeck et al., 2025): occurrences
  match when overlap rate > 0.5, discovered sets are assigned to GT sets so that the number of
  matched occurrences is maximal, and precision / recall / F1 are computed from that assignment.
- **Off-target set**: a discovered motif set that is not assigned to any GT set.
- **Baseline**: a third-party method run for comparison.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Score motif sets against ground truth (Priority: P1)

A researcher has the motif sets found by any method and the true motif sets of the series, and
wants standard, comparable quality scores.

**Why this priority**: every comparison depends on a correct, shared metric.

**Independent Test**: scoring hand-built cases with known answers, and a large random corpus,
gives the same numbers as the reference PROM implementation.

**Acceptance Scenarios**:

1. **Given** discovered sets identical to the GT, **When** scored, **Then** precision = recall =
   F1 = 1.
2. **Given** a discovered occurrence overlapping a GT occurrence by exactly 50 %, **Then** it does
   not match.
3. **Given** more discovered sets than GT sets, **When** the off-target policy is "ignore",
   **Then** unassigned sets do not lower precision; with "penalize" they count as false positives.
4. **Given** a scoring result, **Then** it reports totals, the per-GT-set breakdown and which
   discovered set was assigned to each GT set.
5. **Given** a discovery run whose configuration points to a GT file, **When** it finishes,
   **Then** the result includes its PROM scores.

---

### User Story 2 - Compare ditscover with LoCoMotif and Matrix Profile (Priority: P1)

The researcher runs one command that executes every method on a suite of labelled series under
the same conditions and collects quality and cost per run, with a summary per method and dataset.

**Why this priority**: it is the core request; it shows where the GA stands against the state
of the art.

**Independent Test**: a small suite (synthetic series, 3 methods, 3 seeds) runs unattended and
produces a per-run table and a summary with mean ± std and average rank per method.

**Acceptance Scenarios**:

1. **Given** a benchmark configuration, **When** it runs, **Then** each (method, series, seed) is
   executed in isolation and records precision, recall, F1, wall time, peak memory and status.
2. **Given** a run that exceeds its time limit or crashes, **Then** it is recorded as failed and
   the benchmark continues.
3. **Given** the optional baseline packages are not installed, **Then** the user gets an error
   naming the extra to install, before anything runs.
4. **Given** an interrupted benchmark, **When** it is relaunched with the same output folder,
   **Then** completed runs are skipped.
5. **Given** a scaling section, **Then** time and memory are measured for each method on
   synthetic series of increasing length.
6. **Given** the same configuration twice, **Then** all quality metrics are identical.

---

### User Story 3 - Dashboard report for a discovery run (Priority: P2)

At the end of `ditscover run --plot` (or `res.plot()`), the user opens a single page that leads
with the key numbers and organises the charts in tabs.

**Why this priority**: improves daily use; not required to obtain metrics.

**Independent Test**: open the generated HTML offline; the summary and table are visible at the
top and every tab shows its charts at full size.

**Acceptance Scenarios**:

1. **Given** a result, **Then** the page shows summary cards: stop reason, generations, elapsed
   time, final hypervolume, number of motifs, dropped log events, and precision / recall / F1
   when GT was provided.
2. **Given** the motif table, **When** the user clicks a column header, **Then** rows are sorted
   by support, cohesion, mean length or stretch.
3. **Given** the tabs Motifs / Evolution / Pareto front, **When** the user switches tab, **Then**
   charts render at full width (never collapsed) and the previous tab is hidden.
4. **Given** the page is opened without internet and without `--cdn`, **Then** it works.

---

### User Story 4 - Benchmark report (Priority: P2)

After a benchmark, the user opens one page with the same layout to see which method wins and at
what cost.

**Independent Test**: from a finished benchmark folder, the report command produces the page.

**Acceptance Scenarios**:

1. **Given** benchmark results, **Then** cards show the best method by mean F1, number of runs
   and failures; tabs show Summary (table mean ± std and average rank), Quality (precision /
   recall / F1 per dataset and method) and Cost (time and memory per method, scaling curves).

---

### User Story 5 - Discover motifs in market data from Yahoo Finance (Priority: P3)

The user names a ticker (e.g. `^GSPC`, `AAPL`, `EURUSD=X`), a date range or period and an interval,
and gets motifs on that price series, with dates instead of indices in the report.

**Why this priority**: makes the library attractive to try on real, familiar data; not needed for
the comparison itself.

**Independent Test**: with the download replaced by a recorded sample (no network), a run on a
ticker returns motifs and the report shows dates on the time axis; a second run reads the local
copy instead of downloading.

**Acceptance Scenarios**:

1. **Given** a ticker, range and interval, **When** discovery runs, **Then** the chosen price field
   (adjusted close by default) is used and every occurrence can be shown with its dates.
2. **Given** the same request twice, **Then** the second run uses the local copy and gives the
   same result; a refresh option forces a new download.
3. **Given** an unknown ticker, an empty range or an intraday interval older than the provider
   allows, **Then** a clear error is shown before discovery starts.
4. **Given** the optional finance package is not installed, **Then** the error names the extra to
   install.
5. **Given** a CSV with a date column, **When** that column is declared, **Then** reports also show
   dates.
6. **Given** a benchmark with market-data datasets, **Then** they produce cost figures and number
   of sets, with quality columns empty and excluded from quality rankings.

### Edge Cases

- No discovered sets or no GT sets: precision / recall are 0 where undefined and F1 = 0, never
  a crash.
- GT occurrences of different lengths in one set (variable-length GT) are supported.
- A method returns overlapping occurrences within a set: scored as reported; each occurrence can
  match at most one GT occurrence.
- Matrix Profile finds fewer sets than requested: scored with what it returns.
- A series shorter than a method's minimum requirement: that run is "failed" with the reason.
- A labelled run that fails or times out scores P = R = F1 = 0 in summaries and ranks, so a
  method cannot win by failing on hard series.
- Benchmark folder from an older configuration: runs are skipped only when the configuration
  hash matches.
- Market data has gaps (weekends, holidays, missing rows): motifs are found on consecutive
  observations; missing values follow the configured NaN step.
- An open-ended range ("until today") is resolved to a fixed end date when first downloaded, so
  repeated runs use the same data.

## Requirements *(mandatory)*

### Functional Requirements

**Metrics**

- **FR-001**: The system MUST compute PROM: occurrence matching with overlap rate > 0.5, optimal
  assignment of discovered sets to GT sets maximising matched occurrences, then TP / FP / FN,
  precision, recall and F1.
- **FR-002**: Averaging MUST be selectable (micro by default, macro) and the off-target policy
  MUST be selectable (ignore by default, penalize).
- **FR-003**: Scores MUST include totals, per-GT-set values and the set assignment.
- **FR-004**: Scoring MUST accept motif sets from any source (ditscover, baselines, user lists).
- **FR-005**: A discovery configuration MAY reference a GT file; the result then includes the
  scores (saved and loaded with the result).

**Ground-truth data**

- **FR-006**: GT files MUST hold the series and its motif sets, each occurrence as (start,
  length) in original indices.
- **FR-007**: The synthetic generator MUST be able to produce suites of labelled series
  (number of series, lengths, motif shapes, stretch, noise, seed) and save them as GT files.
- **FR-008**: The benchmark MUST support the univariate datasets of TSMD-Bench (ECG5000, Fungi,
  Plane, Mallat, Symbols), obtained once and cached locally.

**Baselines**

- **FR-009**: LoCoMotif and Matrix Profile (STOMP) MUST be callable through the same interface:
  series + common parameters (`min_len`, `max_len`, number of sets `k`) + method-specific
  parameters → motif sets in original indices.
- **FR-010**: Matrix Profile MUST run with a configurable list of window lengths (default: the
  midpoint of `[min_len, max_len]`); with several lengths, the sets are pooled and the best `k`
  non-overlapping sets are kept.
- **FR-011**: Baselines MUST be optional dependencies; their absence MUST produce an actionable
  error before the benchmark starts.

**Benchmark runner**

- **FR-012**: A benchmark configuration MUST declare datasets, methods with their parameters,
  seeds, `k` policy (number of GT sets per series by default, or fixed; market-data datasets
  always use a fixed `k`, default 5, set per dataset), per-run time limit,
  and an optional scaling section; it MUST be validated before running.
- **FR-013**: Each run MUST execute in an isolated process, measuring wall time and memory:
  total process peak memory and the memory above a baseline taken after imports and warm-up;
  summaries, cost charts and scaling curves compare methods on the latter. Failures and timeouts
  MUST be recorded without stopping the benchmark.
- **FR-014**: Deterministic methods MUST run once per series; stochastic methods once per seed.
- **FR-015**: Outputs MUST include a per-run table, a summary (mean ± std per method and dataset,
  average rank by F1) and the effective configuration. Failed or timed-out runs count as
  P = R = F1 = 0 in the means, in the ranks and in the "best method" choice, and are also
  reported as a failure count per method and dataset.
- **FR-016**: Relaunching with the same output folder and configuration MUST skip finished runs.
- **FR-017**: The benchmark MUST be available as a CLI command and as a Python function.

**Reports**

- **FR-018**: The run report MUST be one self-contained page with summary cards, a sortable
  motif table and tabs Motifs / Evolution / Pareto front (existing charts).
- **FR-019**: Charts in hidden tabs MUST render at full size when shown; the page MUST follow the
  system light / dark preference and work offline unless the CDN option is chosen.
- **FR-020**: The benchmark report MUST reuse the same layout with Summary / Quality / Cost tabs.

**Market data (Yahoo Finance)**

- **FR-021**: A run configuration MUST accept a market-data source instead of a file: ticker,
  start / end dates or a period, interval and price field (default adjusted close).
- **FR-022**: Downloads MUST be stored locally and reused for identical requests; open-ended ranges
  are resolved to fixed dates at first download; a refresh option forces a new download.
- **FR-023**: The provider is an optional dependency; its absence or a failed / empty download MUST
  produce an actionable error before discovery starts.
- **FR-024**: Results MUST carry the timestamp of every observation when the source has them
  (market data, or a declared CSV date column); reports MUST use dates on time axes, in the motif
  table and in tooltips.
- **FR-025**: The benchmark MUST accept market-data datasets (list of tickers + range + interval,
  plus `min_len`, `max_len` and a fixed `k`, default 5, since there is no GT to derive them);
  their runs record cost and number of sets, with empty quality metrics excluded from rankings.

### Key Entities

- **GroundTruth**: series + list of motif sets (occurrences in original indices).
- **MotifSets**: list of motif sets produced by one method on one series.
- **Prom**: TP, FP, FN, precision, recall, F1, per-GT-set values, assignment, averaging, policy.
- **BenchConfig**: datasets, methods + parameters, seeds, `k` policy, time limit, scaling.
- **RunRecord**: method, dataset, series id, seed, scores, wall time, peak memory, memory above
  baseline, status, reason.
- **BenchSummary**: aggregates per method × dataset and average ranks.
- **MarketSource**: ticker, start, end or period, interval, field, refresh flag.
- **Timestamps**: optional date-time label of every observation, carried with the series and the
  result.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Our PROM scores equal the reference implementation on 100 % of a corpus of ≥ 1 000
  random cases (difference < 1e-9).
- **SC-002**: A synthetic suite of 20 series × 3 methods × 3 seeds completes with one command, with
  no manual intervention, in < 15 min on an 8-core laptop.
- **SC-003**: Relaunching an interrupted benchmark re-executes 0 finished runs.
- **SC-004**: Repeating a benchmark yields identical quality metrics in 100 % of runs.
- **SC-005**: In the reports, the best method (benchmark) or the key run figures (run) are visible
  without scrolling at 1280 × 800, and every tab renders its charts at full width.
- **SC-007**: A first-time user gets motifs on a ticker of their choice with one command or two
  lines of code; repeating it works offline from the local copy.
- **SC-006**: On the synthetic variable-length suite, ditscover's mean F1 is reported with its
  standard deviation and rank against both baselines (the result itself is an outcome to
  observe, not a pass / fail target).

## Assumptions

- `k` = number of GT sets of each series for every method (same information for all), unless
  configured otherwise; market-data datasets have no GT and use a fixed `k` (default 5).
- Baseline parameters default to the authors' recommendations (LoCoMotif: `rho` = 0.8, warping
  on; Matrix Profile: default radius of the reference library); no hyper-parameter search in this
  feature.
- The five univariate TSMD-Bench datasets have fixed-length instances; variable-length behaviour
  is evaluated on the synthetic suite.
- Ditscover may use all cores; the number of threads used by each method is recorded with each run.
- Only univariate series, as in feature 001.
- Yahoo Finance data is for personal / research use under the provider's terms; local copies live
  in the user cache and are never committed to the repository. Tests never use the network.
- Intraday intervals are limited by the provider to recent history (e.g. last 60 days).
