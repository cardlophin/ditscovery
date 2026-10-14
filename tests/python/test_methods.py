import sys
from itertools import combinations

import numpy as np
import pytest

import ditscover as ds
from ditscover.bench import methods

PACKAGE = {"locomotif": "locomotif", "stomp": "stumpy"}


@pytest.fixture(scope="module")
def planted():
    x, sets = ds._core.synthetic(1500, [40, 60], 4, 0.2, 0.05, 1)
    return np.asarray(x), sets


def _valid(sets, n, k):
    assert len(sets) <= k
    for occ in sets:
        assert occ
        for s, n_ in occ:
            assert isinstance(s, int) and isinstance(n_, int) and n_ > 0 and s + n_ <= n


@pytest.mark.parametrize("name", ["ditscover", "locomotif", "stomp"])
def test_adapters_return_sets_inside_the_series(name, planted):
    if name in PACKAGE:
        pytest.importorskip(PACKAGE[name])
    x, _ = planted
    params = {"population": 40, "max_generations": 10} if name == "ditscover" else {}
    _valid(methods.run(name, x, 30, 80, 2, 0, params), len(x), 2)


def test_stomp_pools_lengths_into_non_overlapping_sets(planted):
    pytest.importorskip("stumpy")
    x, _ = planted
    sets = methods.run("stomp", x, 30, 80, 3, None, {"lengths": [40, 60]})
    _valid(sets, len(x), 3)
    assert {n for occ in sets for _, n in occ} <= {40, 60}
    for a, b in combinations(sets, 2):
        assert not any(s < t + m and t < s + n for s, n in a for t, m in b)
    for occ in sets:
        assert all(s + n <= t for (s, n), (t, _) in zip(occ, occ[1:], strict=False))


def test_stomp_occurrences_of_a_set_do_not_overlap():
    pytest.importorskip("stumpy")
    rng = np.random.default_rng(0)
    x = rng.normal(0, 0.3, 3000)
    for s in (200, 900, 1700, 2500):
        x[s : s + 60] += 2 * np.sin(np.linspace(0, 4 * np.pi, 60))
    for occ in methods.run("stomp", x, 40, 80, 2, None, {}):
        assert all(s + n <= t for (s, n), (t, _) in zip(occ, occ[1:], strict=False)), occ


def _planted_sine(starts, n=3000, width=60, seed=0):
    rng = np.random.default_rng(seed)
    x = rng.normal(0, 0.3, n)
    for s in starts:
        x[s : s + width] += 2 * np.sin(np.linspace(0, 4 * np.pi, width))
    return x


def _iou(a, b):
    inter = max(0, min(a[0] + a[1], b[0] + b[1]) - max(a[0], b[0]))
    return inter / (a[1] + b[1] - inter)


def test_stomp_returns_several_sets_when_several_motifs_exist():
    pytest.importorskip("stumpy")
    x = _planted_sine((200, 900, 1700, 2500))
    tri = 1.5 * (1 - np.abs(np.linspace(-1, 1, 60)) * 2)
    for s in (500, 1300, 2100, 2800):
        x[s : s + 60] += tri
    sets = methods.run("stomp", x, 50, 70, 2, None, {"max_matches": 4})
    assert sorted(sorted(s for s, _ in occ) for occ in sets) == [
        [195, 895, 1695, 2495],
        [500, 1300, 2100, 2800],
    ], sets


# TSMD-like: distinct shapes around the repeats (LoCoMotif matches flat or noisy background too).
def test_locomotif_hits_a_planted_motif_with_matching_coordinates():
    pytest.importorskip("locomotif")
    rng, t = np.random.default_rng(0), np.linspace(0, 1, 60)
    pieces, starts = [], []
    for i in range(16):
        if i % 4 == 1:
            starts.append(60 * i)
            pieces.append(2 * np.sin(4 * np.pi * t))
        else:
            f, ph, a = rng.uniform(0.3, 3), rng.uniform(0, 2 * np.pi), rng.uniform(0.5, 1.5)
            pieces.append(2 * a * np.cos(2 * np.pi * f * t + ph) + rng.normal(0, 1))
    x = np.concatenate(pieces) + rng.normal(0, 0.05, 960)
    sets = methods.run("locomotif", x, 50, 70, 1, None, {})
    assert sets
    hits = [max(_iou(o, (s, 60)) for o in sets[0]) for s in starts]
    assert sum(h > 0.5 for h in hits) >= 3, (sets, hits)


def test_locomotif_highpass_removes_the_local_level():
    pytest.importorskip("locomotif")
    x = _planted_sine((200, 900, 1700, 2500)) + np.linspace(0, 30, 3000)
    assert methods.run("locomotif", x, 50, 70, 1, None, {"highpass": True})


def test_ditscover_gets_a_stretch_limit_covering_the_ground_truth(tmp_path):
    from ditscover.bench import _method_params, datasets

    files = ds.synthetic_suite(tmp_path, n_series=2, n=3000, lens=(40, 60), occ=4, stretch=0.3)
    for s in datasets._series("syn", files):
        gt = max(max(n for _, n in occ) / min(n for _, n in occ) for occ in s["sets"])
        assert s["max_stretch"] >= gt * 1.1 - 1e-9
        job = {"method": "ditscover", "params": {}, "s": s}
        assert _method_params(job)["max_stretch"] == s["max_stretch"]
        job = {"method": "ditscover", "params": {"max_stretch": 3.0}, "s": s}
        assert _method_params(job)["max_stretch"] == 3.0
        assert "max_stretch" not in _method_params({"method": "stomp", "params": {}, "s": s})


@pytest.mark.parametrize("name", ["locomotif", "stomp"])
def test_short_series_fail_with_a_reason(name):
    with pytest.raises(ValueError, match="shorter"):
        methods.run(name, np.zeros(50), 30, 80, 2, None, {})


def test_missing_package_names_the_extra(monkeypatch):
    monkeypatch.setitem(sys.modules, "stumpy", None)
    with pytest.raises(RuntimeError, match=r"ditscover\[bench\]"):
        methods.check([{"method": "stomp", "label": "stomp", "params": {}}])


def test_unknown_method_or_parameter_fails():
    with pytest.raises(ValueError, match="nope"):
        methods.check([{"method": "nope", "label": "nope", "params": {}}])
    with pytest.raises(ValueError, match="populaton"):
        methods.check([{"method": "ditscover", "label": "d", "params": {"populaton": 4}}])
    pytest.importorskip("locomotif")
    with pytest.raises(ValueError, match="rhoo"):
        methods.check([{"method": "locomotif", "label": "l", "params": {"rhoo": 1}}])
