import statistics
import time

import numpy as np
import pytest

import ditscover as ds


# SC-005: library defaults, one seed, all cores, 20 000 points (release build).
@pytest.mark.slow
def test_defaults_finish_within_ten_seconds_on_20k_points():
    x, _ = ds._core.synthetic(20_000, [60, 80, 100], 8, 0.3, 0.1, 0)
    x = np.asarray(x)
    times = []
    for seed in range(5):
        t0 = time.perf_counter()
        ds.discover(x, min_len=40, max_len=135, max_stretch=2.0, seed=seed, level="error")
        times.append(time.perf_counter() - t0)
    assert statistics.median(times) < 10.0, times
