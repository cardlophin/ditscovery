import json
import os
import signal
import subprocess
import sys
import time

import numpy as np
import pytest

import ditscover as ds


def cli(*args, **kw):
    cmd = [sys.executable, "-m", "ditscover.cli", *map(str, args)]
    return subprocess.run(cmd, capture_output=True, text=True, timeout=120, **kw)


@pytest.fixture
def setup(tmp_path):
    x, _ = ds._core.synthetic(3000, [30], 6, 0.2, 0.1, 1)
    data = tmp_path / "data.csv"
    data.write_text("value\n" + "\n".join(map(repr, x)))
    cfg = tmp_path / "cfg.yaml"
    cfg.write_text(
        f"seed: 1\ndata: {{path: '{data}', column: value}}\n"
        "motif: {min_len: 20, max_len: 40, max_stretch: 2.0}\n"
        "ga: {population: 40}\nstop: {max_generations: 15}\n"
    )
    return tmp_path, cfg, data, np.asarray(x)


def motifs(out):
    return json.loads((out / "results.json").read_text())["motifs"]


def test_run_writes_outputs_and_summary(setup):
    tmp, cfg, _, _ = setup
    out = tmp / "out"
    r = cli("run", cfg, "--out", out)
    assert r.returncode == 0, r.stderr
    for f in ["results.json", "history.jsonl", "config.yaml", "series.csv"]:
        assert (out / f).exists(), f
    assert "support" in r.stdout and "cohesion" in r.stdout
    assert cli("run", cfg, "--out", tmp / "again", "--plot").returncode == 0
    assert motifs(out) == motifs(tmp / "again")
    assert (tmp / "again" / "report.html").stat().st_size > 1_000_000


def test_invalid_config_exits_2(setup):
    tmp, cfg, _, _ = setup
    bad = tmp / "bad.yaml"
    bad.write_text(cfg.read_text() + "output: {foo: 1}\n")
    r = cli("run", bad, "--out", tmp / "o")
    assert r.returncode == 2 and "foo" in r.stderr
    r = cli("run", cfg, "--set", "population=7", "--out", tmp / "o")
    assert r.returncode == 2 and "ga.population" in r.stderr


def test_arguments_override_yaml(setup):
    tmp, cfg, _, _ = setup
    out = tmp / "o"
    r = cli(
        "run",
        cfg,
        "--set",
        "population=20",
        "--seed",
        "9",
        "--set",
        "max_generations=3",
        "--out",
        out,
    )
    assert r.returncode == 0, r.stderr
    text = (out / "config.yaml").read_text()
    assert "population: 20" in text and "seed: 9" in text and "max_generations: 3" in text


def test_data_error_exits_1(setup):
    tmp, cfg, _, _ = setup
    r = cli("run", cfg, "--data", tmp / "missing.csv", "--out", tmp / "o")
    assert r.returncode == 1 and "missing.csv" in r.stderr


def test_sigint_writes_partial_result_and_exits_130(setup):
    tmp, cfg, _, _ = setup
    out = tmp / "o"
    args = ["--set", "max_generations=1000000000", "--set", "stagnation_window=null"]
    cmd = [sys.executable, "-m", "ditscover.cli", "run", str(cfg), *args, "--out", str(out)]
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    time.sleep(1.5)
    p.send_signal(signal.SIGINT)
    p.communicate(timeout=10)
    assert p.returncode == 130
    assert json.loads((out / "results.json").read_text())["stop"] == "cancelled"


def test_verbose_shows_each_generation(setup):
    tmp, cfg, _, _ = setup
    r = cli("run", cfg, "-v", "--set", "max_generations=3", "--out", tmp / "o")
    assert r.returncode == 0
    assert all(f"gen {g} " in r.stderr for g in range(4)), r.stderr


BENCH = (
    "datasets: [{synthetic: {name: syn, n_series: 1, n: 1500, lens: [40], occ: 4}}]\n"
    "methods: [{method: ditscover, params: {population: 20, max_generations: 5}}]\n"
    "seeds: [0]\n"
)


def test_bench_smoke_run(tmp_path):
    cfg = tmp_path / "bench.yaml"
    cfg.write_text(BENCH)
    out = tmp_path / "b"
    r = cli("bench", cfg, "--out", out)
    assert r.returncode == 0, r.stderr
    for f in ["runs.csv", "summary.json", "bench.yaml"]:
        assert (out / f).exists(), f
    assert "ditscover" in r.stdout and "syn" in r.stdout
    assert "[1/1] ditscover syn#0 seed=0 ok F1=" in r.stderr
    assert "population: 20" in (out / "bench.yaml").read_text()


def test_bench_invalid_config_exits_2(tmp_path):
    cfg = tmp_path / "bench.yaml"
    cfg.write_text("datasets: []\nmethods: [{method: ditscover}]\n")
    r = cli("bench", cfg, "--out", tmp_path / "b")
    assert r.returncode == 2 and "datasets" in r.stderr
    cfg.write_text(BENCH)
    r = cli("bench", cfg, "--set", "nope=1", "--out", tmp_path / "b")
    assert r.returncode == 2 and "nope" in r.stderr


def test_bench_missing_extra_exits_1(tmp_path):
    broken = tmp_path / "site" / "locomotif"
    broken.mkdir(parents=True)
    (broken / "__init__.py").write_text("raise ImportError('not installed')\n")
    cfg = tmp_path / "bench.yaml"
    cfg.write_text(BENCH.split("methods")[0] + "methods: [{method: locomotif}]\n")
    env = {**os.environ, "PYTHONPATH": str(tmp_path / "site")}
    r = cli("bench", cfg, "--out", tmp_path / "b", env=env)
    assert r.returncode == 1 and "ditscover[bench]" in r.stderr


FAKE_YFINANCE = """
import numpy as np, pandas as pd
def download(ticker, start=None, end=None, period=None, interval="1d", **kw):
    idx = pd.bdate_range("2024-01-01", periods=300)
    close = 100 + np.cumsum(np.sin(np.arange(300) / 5))
    return pd.DataFrame({"Close": close}, index=idx)
"""


def test_ticker_shortcuts(tmp_path):
    (tmp_path / "site").mkdir()
    (tmp_path / "site" / "yfinance.py").write_text(FAKE_YFINANCE)
    env = {**os.environ, "PYTHONPATH": str(tmp_path / "site"), "DITSCOVER_CACHE": str(tmp_path)}
    args = ["--set", "min_len=10", "--set", "max_len=30", "--set", "max_generations=5"]
    out = tmp_path / "o"
    r = cli("run", "--ticker", "AAPL", "--start", "2024-01-01", "--end", "2025-03-01", *args,
            "--out", out, env=env)  # fmt: skip
    assert r.returncode == 0, r.stderr
    assert "ticker: AAPL" in (out / "config.yaml").read_text()
    assert (out / "series.csv").read_text().startswith("timestamp,value\n2024-01-01,")
    r = cli("run", "--data", "x.csv", "--ticker", "AAPL", *args, env=env)
    assert r.returncode == 2


def test_quiet_and_load_without_source(setup):
    tmp, cfg, data, x = setup
    out = tmp / "o"
    r = cli("run", cfg, "-q", "--out", out)
    assert r.returncode == 0 and r.stderr == ""
    data.unlink()
    res = ds.load(out)
    np.testing.assert_array_equal(res.series, x)
    assert res.motifs and res.history
    res.save(tmp / "copy")
    copy = ds.load(tmp / "copy")
    assert [m.occurrences for m in copy.motifs] == [m.occurrences for m in res.motifs]
    for a, b in zip(copy.motifs, res.motifs, strict=True):
        np.testing.assert_array_equal(a.prototype, b.prototype)
    np.testing.assert_array_equal(copy.series, x)
