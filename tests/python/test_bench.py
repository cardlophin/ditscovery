import csv
import os
import sys
import types
from pathlib import Path

import numpy as np
import pytest

import ditscover as ds
from ditscover.bench import summarize

HERE = Path(__file__).parent


@pytest.fixture
def fake(monkeypatch, tmp_path):
    monkeypatch.setenv("DITSCOVER_METHODS", "fake_methods")
    monkeypatch.setenv("PYTHONPATH", os.pathsep.join([str(HERE), os.environ.get("PYTHONPATH", "")]))
    log = tmp_path / "calls.log"
    monkeypatch.setenv("FAKE_LOG", str(log))
    # warm-up calls use a short series; count only runs on the 1500-point benchmark series
    return lambda: log.read_text().split().count("1500") if log.exists() else 0


def config(methods, n_series=2, **kw):
    synth = {"name": "syn", "n_series": n_series, "n": 1500, "lens": [40], "occ": 4}
    return {"datasets": [{"synthetic": synth}], "methods": methods, "seeds": [0], **kw}


def rows(out):
    with open(out / "runs.csv", newline="") as f:
        return list(csv.DictReader(f))


def test_failures_are_recorded_and_the_benchmark_continues(tmp_path, fake):
    methods = [
        {"method": "fake", "label": "ok"},
        {"method": "fake", "label": "crash", "params": {"mode": "crash"}},
        {"method": "fake", "label": "slow", "params": {"mode": "sleep", "sleep": 30}},
    ]
    b = ds.bench(config(methods, timeout_s=2), out=tmp_path / "b")
    status = {(r["label"], r["status"]) for r in b.runs}
    assert status == {("ok", "ok"), ("crash", "error"), ("slow", "timeout")}
    assert all("boom" in r["reason"] for r in b.runs if r["label"] == "crash")
    assert len(b.runs) == 6
    for f in ("runs.csv", "summary.json", "bench.yaml"):
        assert (tmp_path / "b" / f).exists()


def test_resume_skips_finished_runs(tmp_path, fake):
    out = tmp_path / "b"
    cfg = config([{"method": "fake", "label": "a"}], seeds=[0, 1])
    assert len(ds.bench(cfg, out=out).runs) == 4 and fake() == 4
    assert len(ds.bench(cfg, out=out).runs) == 4 and fake() == 4
    lines = (out / "runs.csv").read_text().splitlines()
    (out / "runs.csv").write_text("\n".join(lines[:-1]) + "\n")
    assert len(ds.bench(cfg, out=out).runs) == 4 and fake() == 5
    assert len(rows(out)) == 4
    changed = config([{"method": "fake", "label": "a", "params": {"sleep": 0.0}}], seeds=[0, 1])
    assert len(ds.bench(changed, out=out).runs) == 4 and fake() == 9


def test_unavailable_methods_fail_before_any_run(tmp_path, fake, monkeypatch):
    with pytest.raises(ValueError, match="nope"):
        ds.bench(config([{"method": "fake"}, {"method": "nope"}]), out=tmp_path / "b")
    monkeypatch.setitem(sys.modules, "stumpy", None)
    with pytest.raises(RuntimeError, match=r"ditscover\[bench\]"):
        ds.bench(config([{"method": "fake"}, {"method": "stomp"}]), out=tmp_path / "b")
    assert fake() == 0 and not (tmp_path / "b" / "runs.csv").exists()


def test_deterministic_methods_run_once_per_series(tmp_path, fake):
    methods = [{"method": "fake"}, {"method": "fake_det"}]
    b = ds.bench(config(methods, seeds=[0, 1, 2]), out=tmp_path / "b")
    count = {m: sum(r["label"] == m for r in b.runs) for m in ("fake", "fake_det")}
    assert count == {"fake": 6, "fake_det": 2}
    assert all(r["seed"] is None for r in b.runs if r["label"] == "fake_det")


def test_repeated_benchmark_gives_identical_quality(tmp_path):
    params = {"population": 20, "max_generations": 5}
    cfg = config([{"method": "ditscover", "params": params}], seeds=[0, 1])
    quality = [
        [(r["precision"], r["recall"], r["f1"]) for r in ds.bench(cfg, out=tmp_path / d).runs]
        for d in ("a", "b")
    ]
    assert quality[0] == quality[1] and all(r["f1"] is not None for r in rows(tmp_path / "a"))


def test_scaling_rows(tmp_path, fake):
    cfg = config([{"method": "fake"}], scaling={"lengths": [1000, 2000], "repeats": 2})
    b = ds.bench(cfg, out=tmp_path / "b")
    scaling = {(s["label"], s["n"]) for s in b.summary["scaling"]}
    assert scaling == {("fake", 1000), ("fake", 2000)}
    assert all("time_s" in s and "delta_mb" in s for s in b.summary["scaling"])
    assert all(r["dataset"] != "scaling" for r in b.summary["by_method_dataset"])


def test_load_bench(tmp_path, fake):
    b = ds.bench(config([{"method": "fake"}]), out=tmp_path / "b")
    loaded = ds.load_bench(tmp_path / "b")
    assert loaded.runs == b.runs and loaded.summary == b.summary


def test_market_data_datasets_report_cost_only(tmp_path, fake, monkeypatch):
    import pandas as pd

    def download(ticker, **kw):
        idx = pd.bdate_range("2024-01-01", periods=400)
        return pd.DataFrame({"Close": np.cumsum(np.sin(np.arange(400) / 7))}, index=idx)

    monkeypatch.setitem(sys.modules, "yfinance", types.SimpleNamespace(download=download))
    monkeypatch.setenv("DITSCOVER_CACHE", str(tmp_path / "cache"))
    market = {"name": "market", "tickers": ["A", "B"], "start": "2024-01-01", "end": "2025-08-01"}
    market |= {"min_len": 10, "max_len": 30}
    cfg = config([{"method": "fake"}, {"method": "fake_det", "params": {"mode": "crash"}}])
    cfg["datasets"].append({"yfinance": market})
    b = ds.bench(cfg, out=tmp_path / "b")
    rows = [r for r in b.runs if r["dataset"] == "market"]
    assert {r["series"] for r in rows} == {"market:A", "market:B"}
    ok = [r for r in rows if r["status"] == "ok"]
    assert all(r["n_sets"] == 5 and r["f1"] is None and r["time_s"] is not None for r in ok)
    by = {(r["label"], r["dataset"]): r for r in b.summary["by_method_dataset"]}
    assert by["fake", "market"]["f1_mean"] is None and by["fake_det", "market"]["failures"] == 2
    assert all("market" not in ranks for ranks in b.summary["rank"].values())


def test_tsmd_series_are_generated_offline_and_cached(tmp_path, monkeypatch):
    from ditscover.bench import datasets

    rng = np.random.default_rng(1)
    y = np.repeat([1.0, 2.0, 3.0, 4.0, 5.0], 20)
    calls = []
    monkeypatch.setattr(
        datasets, "_ucr", lambda name: calls.append(name) or (rng.normal(size=(100, 10)), y)
    )
    monkeypatch.setenv("DITSCOVER_CACHE", str(tmp_path))
    files = datasets.tsmd("ECG5000", "test", 5)
    assert len(files) == 5 and len(datasets.tsmd("ECG5000", "validation", 50)) == 50
    for f in files:
        x, sets = ds.read_truth(f)
        assert 1 <= len(sets) <= 2 and all(len(occ) >= 2 for occ in sets)
        occ = sorted(o for s in sets for o in s)
        assert all(n == 10 and s % 10 == 0 for s, n in occ) and occ[-1][0] + 10 == len(x)
    datasets.tsmd("ECG5000", "test", 3)
    assert calls == ["ECG5000"]


def run(label, series, seed, status, f1, dataset="d", time_s=1.0):
    q = None if f1 is None else f1
    return {
        "method": label,
        "label": label,
        "dataset": dataset,
        "series": series,
        "seed": seed,
        "status": status,
        "precision": q,
        "recall": q,
        "f1": q,
        "n_sets": 1,
        "time_s": time_s,
        "peak_mb": 10.0,
        "delta_mb": 2.0,
    }


def test_summary_scores_failures_as_zero():
    runs = [
        run("A", "s0", 0, "ok", 0.8),
        run("A", "s0", 1, "ok", 0.6),
        run("A", "s1", 0, "timeout", None, time_s=99.0),
        run("A", "s1", 1, "ok", 1.0),
        run("B", "s0", None, "ok", 0.7),
        run("B", "s1", None, "ok", 0.7),
        run("C", "s0", None, "ok", 0.9),
        run("C", "s1", None, "error", None, time_s=99.0),
        run("A", "m0", 0, "ok", None, dataset="market"),
        run("B", "m0", None, "error", None, dataset="market"),
    ]
    s = summarize(runs, unlabelled={"market"})
    by = {(r["label"], r["dataset"]): r for r in s["by_method_dataset"]}
    assert by["A", "d"]["f1_mean"] == pytest.approx(0.6)
    assert by["C", "d"]["f1_mean"] == pytest.approx(0.45)
    assert (by["A", "d"]["failures"], by["C", "d"]["failures"]) == (1, 1)
    assert by["A", "d"]["time_s_mean"] == pytest.approx(1.0)
    assert by["A", "market"]["f1_mean"] is None and by["B", "market"]["failures"] == 1
    assert s["rank"]["A"]["d"] == pytest.approx(2.25)
    assert s["rank"]["B"]["d"] == pytest.approx(1.75)
    assert s["rank"]["C"]["overall"] == pytest.approx(2.0)
    assert "market" not in s["rank"]["A"]
    assert s["best"] == "B"


def test_best_ties_break_on_rank():
    runs = [
        run("A", "s0", None, "ok", 1.0),
        run("A", "s1", None, "ok", 0.0),
        run("B", "s0", None, "ok", 0.5),
        run("B", "s1", None, "ok", 0.5),
        run("C", "s0", None, "ok", 0.0),
        run("C", "s1", None, "ok", 0.2),
    ]
    s = summarize(runs)
    assert (s["rank"]["A"]["overall"], s["rank"]["B"]["overall"]) == (2.0, 1.5)
    assert s["best"] == "B"
    runs[1]["f1"] = 0.1
    assert summarize(runs)["best"] == "A"
