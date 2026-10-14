import contextlib
import json
import sys
import time

import numpy as np

from . import methods


def _maxrss_mb():
    try:
        import resource
    except ImportError:
        return None
    rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    return rss / 2**20 if sys.platform == "darwin" else rss / 1024


# ditscover has no JIT to warm: a one-generation run only spins up the thread pool and memory.
CHEAP = dict(population=4, max_generations=1, min_generations=0, seed_nn=0)


def main():
    job = json.load(sys.stdin)
    args = (job["min_len"], job["max_len"], job["k"], job["seed"], job["params"])
    warm = np.cumsum(np.random.default_rng(0).normal(size=4 * job["max_len"]))
    cheap = job["params"] | CHEAP if job["method"] == "ditscover" else job["params"]
    with contextlib.suppress(Exception):
        methods.run(job["method"], warm, *args[:-1], cheap)
    base = _maxrss_mb()
    t0 = time.perf_counter()
    sets = methods.run(job["method"], job["series"], *args)
    elapsed = time.perf_counter() - t0
    peak = _maxrss_mb()
    delta = None if peak is None else max(0.0, peak - base)
    out = {"sets": sets, "time_s": elapsed, "peak_mb": peak, "delta_mb": delta}
    json.dump({**out, "threads": job["threads"]}, sys.stdout)


if __name__ == "__main__":
    main()
