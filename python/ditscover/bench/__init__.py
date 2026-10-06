import csv
import hashlib
import json
import os
import statistics
import subprocess
import sys
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import numpy as np

from .. import _core, prom
from . import datasets, methods

FIELDS = [
    "method", "label", "dataset", "series", "seed", "cfg_hash", "status", "reason",
    "precision", "recall", "f1", "tp", "fp", "fn", "n_sets", "time_s", "peak_mb", "delta_mb",
    "threads", "n",
]  # fmt: skip
INTS = {"seed", "tp", "fp", "fn", "n_sets", "threads", "n"}
FLOATS = {"precision", "recall", "f1", "time_s", "peak_mb", "delta_mb"}
QUALITY = ("precision", "recall", "f1")
COST = ("time_s", "peak_mb", "delta_mb", "n_sets")
SCALING_LEN = 40


class Bench:
    def __init__(self, out, runs, summary):
        self.out, self.runs, self.summary = Path(out), runs, summary

    def __repr__(self):
        return f"Bench(runs={len(self.runs)}, best={self.summary['best']!r})"

    def plot(self, path=None, cdn=False):
        from .. import report

        return report.bench_report(self, path, cdn)


def _parse(row):
    def value(k, v):
        if v == "" or k not in INTS | FLOATS:
            return None if v == "" else v
        return int(v) if k in INTS else float(v)

    return {k: value(k, v) for k, v in row.items()}


def _read(out):
    path = Path(out) / "runs.csv"
    if not path.exists():
        return []
    with open(path, newline="") as f:
        return [_parse(r) for r in csv.DictReader(f)]


def _write(path, rows, mode):
    with open(path, mode, newline="") as f:
        w = csv.DictWriter(f, FIELDS)
        if mode == "w":
            w.writeheader()
        w.writerows({k: "" if r.get(k) is None else r[k] for k in FIELDS} for r in rows)


def _key(r):
    return (r["label"], r["dataset"], r["series"], r["seed"], r["cfg_hash"])


def _hash(*parts):
    return hashlib.sha256(json.dumps(parts, sort_keys=True).encode()).hexdigest()[:12]


def _k(cfg, s):
    if "k" in s:
        return s["k"]
    return cfg["k"] if isinstance(cfg["k"], int) else len(s["sets"])


def _jobs(cfg, data):
    _, deterministic = methods.registry()
    threads = cfg["threads"] or os.cpu_count()
    for spec, series in zip(cfg["datasets"], data, strict=True):
        name = next(iter(spec.values()))["name"]
        for s in series:
            k = _k(cfg, s)
            for m in cfg["methods"]:
                seeds = [None] if m["method"] in deterministic else cfg["seeds"]
                h = _hash(spec, m, k, cfg["threads"])
                for seed in seeds:
                    yield dict(m, dataset=name, s=s, k=k, seed=seed, cfg_hash=h, threads=threads)


def _scaling_jobs(cfg):
    sc = cfg["scaling"]
    _, deterministic = methods.registry()
    threads = cfg["threads"] or os.cpu_count()
    for n in sc["lengths"]:
        for r in range(sc["repeats"]):
            x, sets = _core.synthetic(n, [SCALING_LEN], 4, 0.2, 0.1, r)
            s = dict(id=f"n={n}#{r}", x=np.asarray(x), sets=sets, min_len=32, max_len=48)
            for m in cfg["methods"]:
                if sc["methods"] is None or m["label"] in sc["methods"]:
                    seed = None if m["method"] in deterministic else cfg["seeds"][0]
                    h = _hash("scaling", n, m, cfg["threads"])
                    yield dict(
                        m, dataset="scaling", s=s, k=1, seed=seed, cfg_hash=h, threads=threads
                    )


def _tail(text):
    lines = [line for line in text.strip().splitlines() if line.strip()]
    return lines[-1] if lines else "no output"


def _execute(job, timeout):
    params = dict(job["params"])
    s = job["s"]
    payload = dict(method=job["method"], params=params, series=s["x"].tolist(), k=job["k"])
    payload |= dict(seed=job["seed"], threads=job["threads"])
    payload |= {k: params.pop(k, s[k]) for k in ("min_len", "max_len")}
    env = {"NUMBA_CACHE_DIR": str(datasets.cache_dir() / "numba"), **os.environ}
    for var in ("RAYON_NUM_THREADS", "NUMBA_NUM_THREADS", "OMP_NUM_THREADS"):
        env[var] = str(job["threads"])
    cmd = [sys.executable, "-m", "ditscover.bench.worker"]
    try:
        p = subprocess.run(
            cmd, input=json.dumps(payload), capture_output=True, text=True, timeout=timeout, env=env
        )
    except subprocess.TimeoutExpired:
        return {"status": "timeout", "reason": f"exceeded {timeout:g} s"}
    if p.returncode != 0:
        return {"status": "error", "reason": _tail(p.stderr)}
    return {"status": "ok", **json.loads(p.stdout)}


def _record(job, res, ev):
    s = job["s"]
    row = {k: job[k] for k in ("method", "label", "dataset", "seed", "cfg_hash", "threads")}
    row |= dict(series=s["id"], n=len(s["x"]), status=res["status"], reason=res.get("reason"))
    if res["status"] == "ok":
        row |= {k: res[k] for k in ("time_s", "peak_mb", "delta_mb")} | {"n_sets": len(res["sets"])}
        if s["sets"] is not None:
            p = prom(res["sets"], s["sets"], ev["average"], ev["off_target"])
            row |= {k: p[k] for k in (*QUALITY, "tp", "fp", "fn")}
    return row


def _progress(i, total, row, stream):
    if stream is None:
        return
    f1 = "-" if row.get("f1") is None else f"{row['f1']:.2f}"
    t = "-" if row.get("time_s") is None else f"{row['time_s']:.1f}s"
    seed = "-" if row["seed"] is None else row["seed"]
    head = f"[{i}/{total}] {row['label']} {row['series']} seed={seed} {row['status']}"
    print(f"{head} F1={f1} {t}", file=stream, flush=True)


def run(cfg, out, progress=None) -> Bench:
    out = Path(out)
    methods.check(cfg["methods"])
    unlabelled = {next(iter(d.values()))["name"] for d in cfg["datasets"] if "yfinance" in d}
    data = [datasets.load(d, out) for d in cfg["datasets"]]
    jobs = list(_jobs(cfg, data))
    if cfg["scaling"]:
        jobs += list(_scaling_jobs(cfg))
    versions = methods.versions(cfg["methods"])
    header = "".join(f"# version {m}: {v}\n" for m, v in versions.items())
    (out / "bench.yaml").write_text(header + _core.bench_yaml(cfg))
    done = {_key(r): r for r in _read(out)}
    for job in jobs:
        job["series"] = job["s"]["id"]
    todo = [j for j in jobs if _key(j) not in done]
    if not (out / "runs.csv").exists():
        _write(out / "runs.csv", [], "w")
    with ThreadPoolExecutor(cfg["jobs"]) as pool:
        results = pool.map(lambda j: (j, _execute(j, cfg["timeout_s"])), todo)
        for i, (job, res) in enumerate(results, 1):
            row = _record(job, res, cfg["eval"])
            _write(out / "runs.csv", [row], "a")
            done[_key(row)] = row
            _progress(i, len(todo), row, progress)
    _write(out / "runs.csv", [done[_key(j)] for j in jobs], "w")
    runs = _read(out)
    summary = summarize(runs, unlabelled) | {"versions": versions, "config": cfg}
    (out / "summary.json").write_text(json.dumps(summary, indent=2))
    return Bench(out, runs, json.loads(json.dumps(summary)))


def load_bench(dir) -> Bench:
    summary = json.loads((Path(dir) / "summary.json").read_text())
    return Bench(dir, _read(dir), summary)


def bench(config, out=None, jobs=None, progress=None, **overrides) -> Bench:
    path, base = (str(config), None) if isinstance(config, str | Path) else (None, config)
    overrides |= {} if jobs is None else {"jobs": jobs}
    cfg = _core.build_bench_config(path, base, overrides, None)
    out = Path(out or "ditscover-bench")
    out.mkdir(parents=True, exist_ok=True)
    return run(cfg, out, progress)


def _stats(values):
    if values is None or not values:
        return None, None
    return statistics.fmean(values), statistics.stdev(values) if len(values) > 1 else 0.0


def _quality(r, k):
    return r[k] if r["status"] == "ok" else 0.0


def _average_ranks(scores):
    return {
        label: 1
        + sum(v > s for v in scores.values())
        + (sum(v == s for v in scores.values()) - 1) / 2
        for label, s in scores.items()
    }


def _ranks(runs, unlabelled):
    per = defaultdict(lambda: defaultdict(list))
    for r in runs:
        if r["dataset"] not in unlabelled:
            per[r["dataset"], r["series"]][r["label"]].append(_quality(r, "f1"))
    ranks = defaultdict(lambda: defaultdict(list))
    for (dataset, _), by_label in per.items():
        means = {label: statistics.fmean(v) for label, v in by_label.items()}
        for label, rank in _average_ranks(means).items():
            ranks[label][dataset].append(rank)
    return {
        label: {d: statistics.fmean(v) for d, v in by_d.items()}
        | {"overall": statistics.fmean([x for v in by_d.values() for x in v])}
        for label, by_d in ranks.items()
    }


def _group(runs, unlabelled):
    groups = defaultdict(list)
    for r in runs:
        groups[r["label"], r["dataset"]].append(r)
    for (label, dataset), rs in groups.items():
        ok = [r for r in rs if r["status"] == "ok"]
        row = dict(label=label, dataset=dataset, n_runs=len(rs), failures=len(rs) - len(ok))
        for k in QUALITY:
            vals = None if dataset in unlabelled else [_quality(r, k) for r in rs]
            row[f"{k}_mean"], row[f"{k}_std"] = _stats(vals)
        for k in COST:
            row[f"{k}_mean"], row[f"{k}_std"] = _stats([r[k] for r in ok if r[k] is not None])
        yield row


def _scaling(runs):
    groups = defaultdict(list)
    for r in runs:
        if r["status"] == "ok":
            groups[r["label"], r["n"]].append(r)

    def median(rs, k):
        vals = [r[k] for r in rs if r[k] is not None]
        return statistics.median(vals) if vals else None

    return [
        dict(label=label, n=n, time_s=median(rs, "time_s"), delta_mb=median(rs, "delta_mb"))
        for (label, n), rs in groups.items()
    ]


def summarize(runs, unlabelled=frozenset()):
    main = [r for r in runs if r["dataset"] != "scaling"]
    rank = _ranks(main, unlabelled)
    labelled = [r for r in main if r["dataset"] not in unlabelled]
    f1 = {
        label: statistics.fmean(_quality(r, "f1") for r in labelled if r["label"] == label)
        for label in rank
    }
    best = max(rank, key=lambda label: (f1[label], -rank[label]["overall"]), default=None)
    scaling = _scaling([r for r in runs if r["dataset"] == "scaling"])
    return dict(
        by_method_dataset=list(_group(main, unlabelled)), rank=rank, best=best, scaling=scaling
    )
