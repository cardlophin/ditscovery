import csv
import hashlib
import itertools
import json
import math
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
    "method", "label", "dataset", "split", "series", "seed", "cfg_hash", "status", "reason",
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

    def stored_sets(self):
        path = self.out / "sets.jsonl"
        if not path.exists():
            return {}
        lines = (json.loads(line) for line in path.read_text().splitlines() if line.strip())
        return {tuple(d["key"]): d["sets"] for d in lines}

    def sets(self, key):
        return self.stored_sets().get(tuple(key))


def split_of(r):
    return r.get("split") or "test"


# Summary key of a dataset on a split (the test split keeps the plain name).
def ds_key(dataset, split):
    return dataset if split in (None, "", "test") else f"{dataset}@{split}"


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
    return (r["label"], r["dataset"], split_of(r), r["series"], r["seed"], r["cfg_hash"])


def _hash(*parts):
    return hashlib.sha256(json.dumps(parts, sort_keys=True).encode()).hexdigest()[:12]


def _k(cfg, s):
    if "k" in s:
        return s["k"]
    return cfg["k"] if isinstance(cfg["k"], int) else len(s["sets"])


def _spec(spec):
    d = next(iter(spec.values()))
    return d["name"], d.get("split", "test")


def _seeds(cfg, m, deterministic):
    return [None] if m["method"] in deterministic else cfg["seeds"]


def _jobs(cfg, data, tuned):
    _, deterministic = methods.registry()
    threads = cfg["threads"] or os.cpu_count()
    for spec, series in zip(cfg["datasets"], data, strict=True):
        name, split = _spec(spec)
        for s in series:
            k = _k(cfg, s)
            for m in cfg["methods"]:
                combo = tuned.get(m["label"], {}).get(name)
                h = _hash(spec, m, k, cfg["threads"], *([] if combo is None else [combo]))
                params = (m.get("params") or {}) | (combo or {})
                for seed in _seeds(cfg, m, deterministic):
                    yield dict(
                        m, params=params, dataset=name, split=split, s=s, k=k, seed=seed,
                        cfg_hash=h, threads=threads,
                    )  # fmt: skip


def _tune_jobs(cfg, out):
    grids = [m for m in cfg["methods"] if m.get("grid")]
    if not cfg["tune"] or not grids:
        return []
    _, deterministic = methods.registry()
    threads = cfg["threads"] or os.cpu_count()
    jobs = []
    for spec in cfg["datasets"]:
        name, _ = _spec(spec)
        for s in datasets.validation(spec, cfg["tune"]["n_series"], out) or []:
            k = _k(cfg, s)
            for m in grids:
                for i, combo in enumerate(methods.combos(m["grid"])):
                    h = _hash(spec, m, combo, k, cfg["threads"], "tune")
                    params = (m.get("params") or {}) | combo
                    jobs += [
                        dict(
                            m,
                            params=params,
                            dataset=name,
                            split="tune",
                            s=s,
                            k=k,
                            seed=seed,
                            cfg_hash=h,
                            threads=threads,
                            combo=i,
                        )  # fmt: skip
                        for seed in _seeds(cfg, m, deterministic)
                    ]
    return jobs


def _choose(combos, scores):
    best = max(sorted(scores), key=lambda i: (statistics.fmean(scores[i]), -i))
    return combos[best]


def _tuned(cfg, jobs, done):
    out = {}
    for m in (m for m in cfg["methods"] if m.get("grid")):
        combos = methods.combos(m["grid"])
        out[m["label"]] = {}
        for spec in cfg["datasets"]:
            name, _ = _spec(spec)
            scores = defaultdict(list)
            for j in jobs:
                r = done.get(_key(j))
                if j["label"] == m["label"] and j["dataset"] == name and r is not None:
                    scores[j["combo"]].append(_quality(r, "f1") or 0.0)
            out[m["label"]][name] = _choose(combos, scores) if scores else combos[0]
    return out


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
                        m, dataset="scaling", split="test", s=s, k=1, seed=seed, cfg_hash=h,
                        threads=threads,
                    )  # fmt: skip


def _tail(text):
    lines = [line for line in text.strip().splitlines() if line.strip()]
    return lines[-1] if lines else "no output"


def _method_params(job):
    params = dict(job.get("params") or {})
    if job["method"] == "ditscover" and "max_stretch" in job["s"]:
        params.setdefault("max_stretch", job["s"]["max_stretch"])
    return params


def _execute(job, timeout):
    params = _method_params(job)
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
    row = {k: job[k] for k in ("method", "label", "dataset", "split", "seed", "cfg_hash")}
    row |= dict(threads=job["threads"], series=s["id"], n=len(s["x"]), status=res["status"])
    row["reason"] = res.get("reason")
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


def _execute_all(jobs, done, cfg, out, progress):
    for job in jobs:
        job["series"] = job["s"]["id"]
    todo = [j for j in jobs if _key(j) not in done]
    with ThreadPoolExecutor(cfg["jobs"]) as pool:
        results = pool.map(lambda j: (j, _execute(j, cfg["timeout_s"])), todo)
        for i, (job, res) in enumerate(results, 1):
            row = _record(job, res, cfg["eval"])
            _write(out / "runs.csv", [row], "a")
            if cfg["save_sets"] and res["status"] == "ok":
                with open(out / "sets.jsonl", "a") as f:
                    f.write(json.dumps({"key": list(_key(row)), "sets": res["sets"]}) + "\n")
            done[_key(row)] = row
            _progress(i, len(todo), row, progress)


def run(cfg, out, progress=None) -> Bench:
    out = Path(out)
    methods.check(cfg["methods"])
    unlabelled = {next(iter(d.values()))["name"] for d in cfg["datasets"] if "yfinance" in d}
    data = [datasets.load(d, out) for d in cfg["datasets"]]
    tune = _tune_jobs(cfg, out)
    n_eval = sum(1 for _ in _jobs(cfg, data, {}))
    n_eval += sum(1 for _ in _scaling_jobs(cfg)) if cfg["scaling"] else 0
    if len(tune) + n_eval > cfg["max_runs"]:
        raise ValueError(
            f"the benchmark needs {len(tune) + n_eval} runs (tuning {len(tune)} + evaluation "
            f"{n_eval}) > max_runs = {cfg['max_runs']}; shrink the grids or tune.n_series"
        )
    versions = methods.versions(cfg["methods"])
    header = "".join(f"# version {m}: {v}\n" for m, v in versions.items())
    (out / "bench.yaml").write_text(header + _core.bench_yaml(cfg))
    done = {_key(r): r for r in _read(out)}
    if not (out / "runs.csv").exists():
        _write(out / "runs.csv", [], "w")
    _execute_all(tune, done, cfg, out, progress)
    tuned = _tuned(cfg, tune, done)
    jobs = list(_jobs(cfg, data, tuned))
    if cfg["scaling"]:
        jobs += list(_scaling_jobs(cfg))
    _execute_all(jobs, done, cfg, out, progress)
    _write(out / "runs.csv", [done[_key(j)] for j in tune + jobs], "w")
    runs = _read(out)
    summary = summarize(runs, unlabelled) | {"versions": versions, "config": cfg, "tuned": tuned}
    if cfg["stats"]:
        summary["tests"] = paired_tests(runs, unlabelled)
        if any(m["label"] == "full" for m in cfg["methods"]):
            summary["ablation"] = ablation(runs, unlabelled)
            summary["decisions"] = decisions(summary["ablation"])
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
            per[ds_key(r["dataset"], split_of(r)), r["series"]][r["label"]].append(
                _quality(r, "f1")
            )
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
        groups[r["label"], r["dataset"], split_of(r)].append(r)
    for (label, dataset, split), rs in groups.items():
        ok = [r for r in rs if r["status"] == "ok"]
        row = dict(label=label, dataset=dataset, split=split, n_runs=len(rs))
        row["failures"] = len(rs) - len(ok)
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
    main = [r for r in runs if r["dataset"] != "scaling" and split_of(r) != "tune"]
    rank = _ranks(main, unlabelled)
    labelled = [r for r in main if r["dataset"] not in unlabelled]
    test = [r for r in labelled if split_of(r) == "test"] or labelled
    f1 = {
        label: statistics.fmean([_quality(r, "f1") for r in test if r["label"] == label] or [0.0])
        for label in rank
    }
    best = max(rank, key=lambda label: (f1[label], -rank[label]["overall"]), default=None)
    scaling = _scaling([r for r in runs if r["dataset"] == "scaling"])
    return dict(
        by_method_dataset=list(_group(main, unlabelled)), rank=rank, best=best, scaling=scaling
    )


# Seed-averaged F1 per (dataset, split) → label → series; failures count as 0.
def per_series(runs, unlabelled=frozenset()):
    raw = defaultdict(lambda: defaultdict(lambda: defaultdict(list)))
    for r in runs:
        skip = r["dataset"] in unlabelled or r["dataset"] == "scaling" or split_of(r) == "tune"
        if skip or (r["status"] == "ok" and r["f1"] is None):
            continue
        raw[r["dataset"], split_of(r)][r["label"]][r["series"]].append(_quality(r, "f1"))
    return {
        key: {label: {s: statistics.fmean(v) for s, v in ser.items()} for label, ser in by.items()}
        for key, by in raw.items()
    }


def wilcoxon(x, y):
    from scipy.stats import wilcoxon as test

    if all(a == b for a, b in zip(x, y, strict=True)):
        return 0.0, 1.0
    res = test(x, y, zero_method="zsplit")
    p = float(res.pvalue)
    return float(res.statistic), 1.0 if math.isnan(p) else p


def holm(rows):
    order = sorted(range(len(rows)), key=lambda i: rows[i]["p"])
    running = 0.0
    for rank, i in enumerate(order):
        running = max(running, min(1.0, (len(rows) - rank) * rows[i]["p"]))
        rows[i]["p_holm"] = running
    return rows


GUARANTEED = {"labelling": "output guarantee (FR-025): measured, never deleted"}


def _bootstrap(diffs, n=1000):
    d = np.asarray(diffs, dtype=float)
    means = np.random.default_rng(0).choice(d, (n, len(d))).mean(axis=1)
    return float(np.quantile(means, 0.025)), float(np.quantile(means, 0.975))


def _component(label):
    if label == "v1":
        return "v1"
    return label.removeprefix("full-") if label.startswith("full-") else None


# ΔF1 = F1(variant) − F1(full) per (component, dataset, split), seed-averaged per series; Holm
# across the components of each (dataset, split); v1 is a separate comparison.
def ablation(runs, unlabelled=frozenset()):
    out = []
    for (dataset, split), by_label in per_series(runs, unlabelled).items():
        full = by_label.get("full")
        if full is None:
            continue
        rows = []
        for label, ser in by_label.items():
            common = sorted(set(full) & set(ser))
            if _component(label) is None or not common:
                continue
            x, y = [ser[s] for s in common], [full[s] for s in common]
            diffs = [a - b for a, b in zip(x, y, strict=True)]
            stat, p = wilcoxon(x, y)
            lo, hi = _bootstrap(diffs)
            rows.append(dict(component=_component(label), dataset=dataset, split=split))
            rows[-1] |= dict(n=len(common), delta_f1=statistics.fmean(diffs), ci_low=lo, ci_high=hi)
            rows[-1] |= dict(statistic=stat, p=p)
        holm([r for r in rows if r["component"] != "v1"])
        for r in rows:
            r.setdefault("p_holm", r["p"])
        out += rows
    return out


# Validation rows only: keep a component iff removing it lowers F1 significantly on >= 1 dataset
# and raises it significantly on none. For v1, `keep` means the previous model is not beaten.
def decisions(rows, alpha=0.05):
    val = [r for r in rows if r["split"] == "validation"]
    out = []
    for comp in dict.fromkeys(r["component"] for r in val):
        rs = [r for r in val if r["component"] == comp]
        lower = [r["dataset"] for r in rs if r["delta_f1"] < 0 and r["p_holm"] < alpha]
        higher = [r["dataset"] for r in rs if r["delta_f1"] > 0 and r["p_holm"] < alpha]
        beaten = bool(lower) and not higher
        if comp in GUARANTEED:
            keep, reason = True, GUARANTEED[comp]
        elif comp == "v1":
            keep = not beaten
            reason = f"full better on {lower}, worse on {higher}"
        else:
            keep = beaten
            reason = f"removal lowers F1 on {lower}, raises it on {higher}"
        out.append(dict(component=comp, keep=keep, reason=reason))
    return out


def paired_tests(runs, unlabelled=frozenset()):
    out = []
    for (dataset, split), by_label in per_series(runs, unlabelled).items():
        rows = []
        for a, b in itertools.combinations(by_label, 2):
            common = sorted(set(by_label[a]) & set(by_label[b]))
            if not common:
                continue
            x, y = [by_label[a][s] for s in common], [by_label[b][s] for s in common]
            stat, p = wilcoxon(x, y)
            rows.append(dict(dataset=dataset, split=split, a=a, b=b, n=len(common)))
            rows[-1] |= dict(statistic=stat, p=p)
        out += holm(rows)
    return out
