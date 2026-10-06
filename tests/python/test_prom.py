import json
import math
import random

import numpy as np
import pytest

import ditscover as ds


def test_prom_on_lists():
    gt = [[(0, 10), (50, 10)]]
    r = ds.prom(gt, gt)
    assert (r["precision"], r["recall"], r["f1"]) == (1.0, 1.0, 1.0)
    assert (r["tp"], r["fp"], r["fn"]) == (2, 0, 0)
    assert r["sets"][0]["found"] == 0
    assert ds.prom([[(5, 10)]], [[(0, 10)]])["f1"] == 0.0
    off = ds.prom([[(0, 10)], [(80, 10)]], [[(0, 10)]], off_target="penalize")
    assert off["precision"] == 0.5
    with pytest.raises(ValueError, match="average"):
        ds.prom(gt, gt, average="weighted")


def test_prom_on_a_result():
    x, sets = ds._core.synthetic(3000, [30], 6, 0.2, 0.1, 1)
    res = ds.discover(
        np.asarray(x), min_len=20, max_len=40, population=40, max_generations=10, seed=3
    )
    assert ds.prom(res, sets) == ds.prom([m.occurrences for m in res.motifs], sets)


def test_read_truth(tmp_path):
    path = tmp_path / "gt.json"
    path.write_text(json.dumps({"series": [0, 1, None, 3, 4], "sets": [[[0, 2], [3, 2]]]}))
    x, sets = ds.read_truth(path)
    assert x.shape == (5,) and math.isnan(x[2])
    assert sets == [[(0, 2), (3, 2)]]


def _case(rng):
    slots = rng.sample(range(20), rng.randint(1, 10))
    gt = [[] for _ in range(rng.randint(1, 4))]
    for k, s in enumerate(slots):
        gt[k % len(gt)].append((s * 20, rng.randint(8, 20)))
    gt = [g for g in gt if g]
    found = [
        [
            (max(0, rng.choice(slots) * 20 + rng.randint(-6, 6)), rng.randint(6, 22))
            for _ in range(rng.randint(1, 5))
        ]
        for _ in range(rng.randint(0, 6))
    ]
    return gt, found


def _reference(prom_ref, gt, found, average, penalize):
    ends = lambda sets: [[(s, s + n) for s, n in occ] for occ in sets]  # noqa: E731
    m, _, _ = prom_ref.matching_matrix(ends(gt), ends(found))
    fns = [f"{average}_averaged_{k}" for k in ("precision", "recall", "f1")]
    p = getattr(prom_ref, fns[0])(m, penalize_off_target=penalize)
    r = getattr(prom_ref, fns[1])(m)
    f = getattr(prom_ref, fns[2])(m, penalize_off_target=penalize)
    return [0.0 if np.isnan(v) else float(v) for v in (p, r, f)]


def test_prom_matches_the_reference_implementation():
    prom_ref = pytest.importorskip("tsmd_evaluation.prom")
    rng = random.Random(0)
    for _ in range(1500):
        gt, found = _case(rng)
        for average in ("micro", "macro"):
            for off in ("ignore", "penalize"):
                ours = ds.prom(found, gt, average=average, off_target=off)
                ref = _reference(prom_ref, gt, found, average, off == "penalize")
                got = [ours["precision"], ours["recall"], ours["f1"]]
                assert np.allclose(got, ref, atol=1e-9, rtol=0), (gt, found, average, off)
