import _thread
import json
import threading
import time
from itertools import combinations

import numpy as np
import pandas as pd
import pytest

import ditscover as ds

FAST = dict(min_len=20, max_len=40, max_stretch=2.0, population=40, max_generations=20, seed=3)


@pytest.fixture(scope="module")
def series():
    x, _ = ds._core.synthetic(3000, [30], 6, 0.2, 0.1, 1)
    return np.asarray(x)


def occurrences(res):
    return [m.occurrences for m in res.motifs]


def coverage_jaccard(a, b):
    sa = {i for s, n in a for i in range(s, s + n)}
    sb = {i for s, n in b for i in range(s, s + n)}
    return len(sa & sb) / len(sa | sb)


def test_inputs_are_equivalent(series, tmp_path):
    path = tmp_path / "s.csv"
    path.write_text("value\n" + "\n".join(map(repr, series.tolist())))
    ref = occurrences(ds.discover(series, **FAST))
    assert occurrences(ds.discover(series.tolist(), **FAST)) == ref
    assert occurrences(ds.discover(pd.Series(series), **FAST)) == ref
    assert occurrences(ds.discover(path, **FAST)) == ref
    assert occurrences(ds.discover(str(path), column="value", **FAST)) == ref


def test_result_and_motif_attributes(series):
    res = ds.discover(series, **FAST)
    assert res.stop in {"max_generations", "time_budget", "stagnation", "cancelled"}
    assert res.config["seed"] == 3 and res.config["motif"]["min_len"] == 20
    np.testing.assert_array_equal(res.series, series)
    assert 1 <= len(res.motifs) <= 5 and res.front
    for m in res.motifs:
        assert all(isinstance(s, int) and isinstance(n, int) for s, n in m.occurrences)
        assert isinstance(m.prototype, np.ndarray) and m.prototype.ndim == 1
        assert m.aligned.shape == (m.support, len(m.prototype))
        assert m.support == len(m.occurrences)
        assert m.cohesion >= 0 and m.stretch >= 1 and 20 <= m.mean_len <= 40
        assert len(m.objectives) == 3
    assert all(m.aligned is None for m in res.front)
    keys = {"gen", "elapsed_s", "evals", "hv", "front_size", "best", "mean", "front"}
    assert res.history and keys <= set(res.history[0])


def test_top_k_motifs_are_distinct(series):
    res = ds.discover(series, **FAST)
    for a, b in combinations(occurrences(res), 2):
        assert coverage_jaccard(a, b) <= 0.3


def test_same_seed_same_result(series):
    a, b = ds.discover(series, **FAST), ds.discover(series, **FAST)
    assert occurrences(a) == occurrences(b)
    for ma, mb in zip(a.motifs, b.motifs, strict=True):
        np.testing.assert_array_equal(ma.prototype, mb.prototype)


def test_errors_are_actionable(tmp_path):
    noise = np.random.default_rng(0).normal(size=200)
    with pytest.raises(ValueError, match="motif.max_len"):
        ds.discover(noise, min_len=10, max_len=5)
    with pytest.raises(ValueError, match="min_len"):
        ds.discover(noise[:30], min_len=20, max_len=25)
    with pytest.raises(ValueError, match="nope"):
        ds.discover(noise, min_len=10, max_len=20, nope=1)
    with pytest.raises(OSError):
        ds.discover(tmp_path / "missing.csv", min_len=5, max_len=10)


def test_preprocessing_reports_original_indices():
    x, planted = ds._core.synthetic(6000, [160], 6, 0.1, 0.05, 2)
    x = np.asarray(x) + np.linspace(0, 5, len(x))
    gaps = [i for i in range(10, 6000, 97) if not any(s <= i < s + n for s, n in planted[0])]
    x[gaps] = np.nan
    steps = [{"nan": "interpolate"}, {"detrend": "linear"}, {"paa": 4}]
    res = ds.discover(x, preprocess=steps, min_len=30, max_len=50, max_stretch=1.5, seed=1)
    found = [o for m in res.motifs for o in m.occurrences]
    assert all(s % 4 == 0 and n % 4 == 0 and s + n <= len(x) for s, n in found)

    def iou(a, b):
        inter = max(0, min(a[0] + a[1], b[0] + b[1]) - max(a[0], b[0]))
        return inter / (a[1] + b[1] - inter)

    hits = [p for p in planted[0] if any(iou(f, p) >= 0.5 for f in found)]
    assert len(hits) >= 4, (res.motifs[0].occurrences, planted[0])


def test_ground_truth_metrics_are_saved_and_loaded(tmp_path):
    x, sets = ds._core.synthetic(3000, [30], 6, 0.2, 0.1, 1)
    gt = tmp_path / "gt.json"
    gt.write_text(json.dumps({"series": x, "sets": sets}))
    res = ds.discover(np.asarray(x), ground_truth=gt, **FAST)
    assert res.metrics["tp"] + res.metrics["fn"] == 6
    assert 0.0 <= res.metrics["f1"] <= 1.0
    res.save(tmp_path / "run")
    assert ds.load(tmp_path / "run").metrics == res.metrics
    assert ds.discover(np.asarray(x), **FAST).metrics is None


def test_ctrl_c_returns_partial_result(series):
    timer = threading.Timer(0.5, _thread.interrupt_main)
    timer.start()
    t0 = time.monotonic()
    res = ds.discover(series, **{**FAST, "max_generations": 10**9, "stagnation_window": None})
    assert res.stop == "cancelled"
    assert time.monotonic() - t0 < 1.5
    assert res.motifs
