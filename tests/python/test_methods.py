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
