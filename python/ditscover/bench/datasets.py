import json
import math
import os
import urllib.request
import zipfile
from pathlib import Path

import numpy as np

from .. import read_truth, synthetic_suite

URL = "https://www.timeseriesclassification.com/aeon-toolkit/{}.zip"
SPLITS = {"validation": 50, "test": 200}
BAND = 0.2


def cache_dir() -> Path:
    return Path(os.environ.get("DITSCOVER_CACHE", Path.home() / ".cache" / "ditscover"))


def _ucr(name):
    path = cache_dir() / "ucr" / f"{name}.zip"
    if not path.exists():
        path.parent.mkdir(parents=True, exist_ok=True)
        part = path.with_suffix(".part")
        urllib.request.urlretrieve(URL.format(name), part)
        part.rename(path)
    with zipfile.ZipFile(path) as z:
        rows = np.vstack([np.loadtxt(z.open(f"{name}_{s}.txt")) for s in ("TRAIN", "TEST")])
    x = rows[:, 1:]
    return (x - x.mean(axis=1, keepdims=True)) / x.std(axis=1, keepdims=True), rows[:, 0]


# Port of tsmd_evaluation.benchmark_generation.generate_tsmd_benchmark_ts: g repeating classes
# with >= 2 instances each, one instance of every other class, alternating motif / non-motif.
def _tsmd_series(x, y, g, rng):
    labels, counts = np.unique(y, return_counts=True)
    rep = rng.choice(labels[counts > 1], size=g, replace=False)
    others = rng.permutation([rng.choice(np.flatnonzero(y == c)) for c in labels if c not in rep])
    motifs = [i for c in rep for i in rng.choice(np.flatnonzero(y == c), 2, replace=False)]
    rest = np.setdiff1d(np.flatnonzero(np.isin(y, rep)), motifs)
    extra = rng.choice(rest, min(len(rest), max(0, len(others) + 1 - len(motifs))), replace=False)
    order = rng.permutation(np.concatenate([motifs, extra]).astype(int))
    pieces, sets, pos = [], {c: [] for c in rep}, 0
    for i in range(min(len(others), len(order) - 1)):
        for j, motif in ((order[i], True), (others[i], False)):
            if motif:
                sets[y[j]].append((pos, x.shape[1]))
            pieces.append(x[j])
            pos += x.shape[1]
    sets[y[order[-1]]].append((pos, x.shape[1]))
    pieces.append(x[order[-1]])
    return np.concatenate(pieces), [sets[c] for c in rep]


def _generate(name, x, y, root):
    rng = np.random.default_rng(0)
    groups = [np.flatnonzero(y == c) for c in np.unique(y)]
    val = np.concatenate([rng.choice(g, round(0.2 * len(g)), replace=False) for g in groups])
    test = np.setdiff1d(np.arange(len(y)), val)
    g_max = (len(groups) + 1) // 3
    for split, idx in (("validation", val), ("test", test)):
        (root / split).mkdir(parents=True, exist_ok=True)
        for i in range(SPLITS[split]):
            series, sets = _tsmd_series(x[idx], y[idx], int(rng.integers(1, g_max + 1)), rng)
            gt = {"name": f"{name}-{split}#{i}", "series": series.tolist(), "sets": sets}
            (root / split / f"{i}.json").write_text(json.dumps(gt))


def tsmd(name, split, n_series) -> list[Path]:
    root = cache_dir() / "tsmd" / name
    if not (root / "test" / f"{SPLITS['test'] - 1}.json").exists():
        _generate(name, *_ucr(name), root)
    return [root / split / f"{i}.json" for i in range(n_series)]


# min_len / max_len of a labelled dataset: its GT occurrence lengths widened by BAND, so that
# fixed-length classes (TSMD-Bench) still leave the methods room (LoCoMotif finds nothing at
# l_min == l_max).
def _series(name, files):
    data = [read_truth(f) for f in files]
    lens = [n for _, sets in data for occ in sets for _, n in occ]
    lo, hi = math.floor(min(lens) * (1 - BAND)), math.ceil(max(lens) * (1 + BAND))
    return [
        dict(id=f"{name}#{i}", x=x, sets=sets, min_len=lo, max_len=hi)
        for i, (x, sets) in enumerate(data)
    ]


def load(spec, out):
    kind, d = next(iter(spec.items()))
    if kind == "synthetic":
        return _series(d["name"], synthetic_suite(Path(out) / "datasets" / d["name"], **d))
    if kind == "tsmd":
        return _series(d["name"], tsmd(d["name"], d["split"], d["n_series"]))
    if kind == "files":
        return _series(d["name"], [Path(p) for p in d["paths"]])
    return [_market(d, t) for t in d["tickers"]]


# Unlabelled (cost only); gaps are interpolated linearly because the baselines reject NaN.
def _market(d, ticker):
    from .. import sources

    x, _, _ = sources.fetch(d | {"ticker": ticker, "refresh": False})
    ok = ~np.isnan(x)
    x = np.interp(np.arange(len(x)), np.flatnonzero(ok), x[ok])
    lens = dict(min_len=d["min_len"], max_len=d["max_len"], k=d["k"])
    return dict(id=f"{d['name']}:{ticker}", x=x, sets=None, **lens)
