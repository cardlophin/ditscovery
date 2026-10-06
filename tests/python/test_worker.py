import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

import ditscover as ds

HERE = Path(__file__).parent


def worker(job):
    env = {**os.environ, "DITSCOVER_METHODS": "fake_methods", "PYTHONPATH": str(HERE)}
    cmd = [sys.executable, "-m", "ditscover.bench.worker"]
    return subprocess.run(
        cmd, input=json.dumps(job), capture_output=True, text=True, timeout=120, env=env
    )


def job(method, **params):
    x, _ = ds._core.synthetic(1500, [30], 4, 0.2, 0.1, 1)
    return {
        "method": method,
        "params": params,
        "series": x,
        "k": 2,
        "seed": 1,
        "min_len": 20,
        "max_len": 40,
        "threads": 1,
    }


def test_worker_reports_sets_time_and_memory():
    r = worker(job("ditscover", population=20, max_generations=5))
    assert r.returncode == 0, r.stderr
    out = json.loads(r.stdout)
    assert 0 < len(out["sets"]) <= 2 and out["time_s"] > 0 and out["threads"] == 1
    if sys.platform == "win32":
        assert out["peak_mb"] is None and out["delta_mb"] is None
    else:
        assert out["peak_mb"] > 0 and 0 <= out["delta_mb"] <= out["peak_mb"]


def test_warm_up_is_not_timed():
    r = worker(job("fake", mode="slow_first", sleep=1.0))
    assert r.returncode == 0, r.stderr
    assert json.loads(r.stdout)["time_s"] < 0.5


def test_failure_exits_with_the_reason():
    r = worker(job("fake", mode="crash"))
    assert r.returncode != 0
    assert "boom" in r.stderr


@pytest.mark.parametrize("bad", ["{", json.dumps({"method": "fake"})])
def test_invalid_job_fails(bad):
    env = {**os.environ, "DITSCOVER_METHODS": "fake_methods", "PYTHONPATH": str(HERE)}
    cmd = [sys.executable, "-m", "ditscover.bench.worker"]
    r = subprocess.run(cmd, input=bad, capture_output=True, text=True, timeout=60, env=env)
    assert r.returncode != 0
