import functools
import importlib
import inspect
import math
import os
from importlib.metadata import PackageNotFoundError, version

import numpy as np

from .. import _core, discover

EXTRA = "pip install 'ditscover[bench]'"
PACKAGES = {"locomotif": ("locomotif", "dtai-locomotif"), "stomp": ("stumpy", "stumpy")}


def _need(x, max_len, name):
    if len(x) < 2 * max_len:
        raise ValueError(f"{name}: series of length {len(x)} is shorter than 2·max_len")


def ditscover(x, min_len, max_len, k, seed=None, **params):
    params = {"level": "error", "history": False, **params}
    res = discover(x, min_len=min_len, max_len=max_len, top_k=k, seed=seed or 0, **params)
    return [m.occurrences for m in res.motifs]


def locomotif(x, min_len, max_len, k, seed=None, rho=0.8, warping=True, overlap=0.0):
    _need(x, max_len, "locomotif")
    from locomotif.locomotif import apply_locomotif

    z = ((x - x.mean()) / x.std()).reshape(-1, 1)
    found = apply_locomotif(z, min_len, max_len, rho=rho, nb=k, overlap=overlap, warping=warping)
    return [sorted((int(b), int(e - b)) for b, e in occ) for _, occ in found]


@functools.cache
def _stumpy():
    import stumpy
    from stumpy import cache

    # numba's on-disk cache (NUMBA_CACHE_DIR) spares each isolated worker the ~15 s JIT compile
    for module, name in cache.get_njit_funcs():
        getattr(importlib.import_module(f"stumpy.{module}"), name).enable_caching()
    # exclusion zone = m (default m/4): matches of one set must not overlap, like GT occurrences
    stumpy.config.STUMPY_EXCL_ZONE_DENOM = 1
    return stumpy


def stomp(x, min_len, max_len, k, seed=None, lengths=None, max_matches=20):
    _need(x, max_len, "stomp")
    stumpy = _stumpy()

    candidates = []
    for m in lengths or [round((min_len + max_len) / 2)]:
        mp = stumpy.stump(x, m)
        dist, idx = stumpy.motifs(x, mp[:, 0].astype(float), max_motifs=k, max_matches=max_matches)
        for d, i in zip(dist, idx, strict=True):
            keep = ~np.isnan(d) & (i >= 0)
            if keep.sum() >= 2:
                candidates.append(
                    (np.mean(d[keep]) / math.sqrt(m), sorted(int(s) for s in i[keep]), m)
                )
    chosen = []
    for _, starts, m in sorted(candidates, key=lambda c: c[0]):
        occ = [(s, m) for s in starts]
        clash = any(s < t + n and t < s + m for s, _ in occ for kept in chosen for t, n in kept)
        if not clash and len(chosen) < k:
            chosen.append(occ)
    return chosen


METHODS = {"ditscover": ditscover, "locomotif": locomotif, "stomp": stomp}
DETERMINISTIC = {"locomotif", "stomp"}


def _plugin():
    name = os.environ.get("DITSCOVER_METHODS")
    return importlib.import_module(name) if name else None


def registry():
    plugin = _plugin()
    extra = getattr(plugin, "METHODS", {})
    return {**METHODS, **extra}, DETERMINISTIC | getattr(plugin, "DETERMINISTIC", set())


def _version(name):
    if name == "ditscover":
        return _core.__version__
    try:
        return version(PACKAGES[name][1])
    except (KeyError, PackageNotFoundError):
        return None


def versions(specs):
    return {s["method"]: _version(s["method"]) for s in specs}


def check(specs):
    known, _ = registry()
    for s in specs:
        name, params = s["method"], s.get("params") or {}
        if name not in known:
            raise ValueError(f"method `{name}`: unknown (available: {', '.join(known)})")
        if name in PACKAGES:
            try:
                importlib.import_module(PACKAGES[name][0])
            except ImportError as e:
                raise RuntimeError(f"method `{name}` needs {PACKAGES[name][1]}: {EXTRA}") from e
        if name == "ditscover":
            _core.build_config(None, None, {"min_len": 1, "max_len": 1, **params}, None)
            continue
        try:
            inspect.signature(known[name]).bind(None, 1, 1, 1, None, **params)
        except TypeError as e:
            raise ValueError(f"method `{s.get('label', name)}` params: {e}") from e


def run(name, x, min_len, max_len, k, seed, params):
    fn = registry()[0][name]
    x = np.asarray(x, dtype=float)
    return [
        [(int(s), int(n)) for s, n in occ] for occ in fn(x, min_len, max_len, k, seed, **params)
    ]
