import argparse
import sys
from pathlib import Path

from . import Result, _core, load, market
from .bench import load_bench
from .bench import run as run_bench

DEFAULT_OUT = "ditscover-out"
DEFAULT_BENCH = "ditscover-bench"


def _parser():
    p = argparse.ArgumentParser(prog="ditscover")
    sub = p.add_subparsers(dest="cmd", required=True)
    run = sub.add_parser("run", help="discover motifs and save the results")
    run.add_argument("config", nargs="?", help="YAML configuration")
    source = run.add_mutually_exclusive_group()
    source.add_argument("--data", help="CSV/TSV file (data.path)")
    source.add_argument("--ticker", help="Yahoo Finance symbol, needs ditscover[finance]")
    run.add_argument("--start", help="first date YYYY-MM-DD (data.start)")
    run.add_argument("--end", help="end date YYYY-MM-DD, exclusive (data.end)")
    run.add_argument("--interval", help="bar size, e.g. 1d, 1wk, 1h (data.interval)")
    run.add_argument("--column", help="column name or index (data.column)")
    run.add_argument("--seed", help="random seed")
    run.add_argument("--out", help=f"output directory (default ./{DEFAULT_OUT})")
    run.add_argument("--set", action="append", default=[], metavar="KEY=VALUE")
    run.add_argument("--plot", action="store_true", help="also write report.html")
    run.add_argument("--cdn", action="store_true", help="load plotly.js from the CDN")
    level = run.add_mutually_exclusive_group()
    level.add_argument("-q", "--quiet", action="store_true")
    level.add_argument("-v", "--verbose", action="store_true")
    bench = sub.add_parser("bench", help="compare methods on labelled datasets")
    bench.add_argument("config", help="benchmark YAML")
    bench.add_argument("--out", help=f"output directory (default ./{DEFAULT_BENCH})")
    bench.add_argument("--set", action="append", default=[], metavar="KEY=VALUE")
    bench.add_argument("--jobs", type=int, help="runs in parallel (timings less reliable)")
    bench.add_argument("--plot", action="store_true", help="also write report.html")
    bench.add_argument("--cdn", action="store_true", help="load plotly.js from the CDN")
    bench.add_argument("-q", "--quiet", action="store_true", help="no progress lines")
    plot = sub.add_parser("plot", help="write the HTML report of a saved result")
    plot.add_argument("dir")
    plot.add_argument(
        "--kind", default="all", choices=["all", "motifs", "overlay", "evolution", "front"]
    )
    plot.add_argument("--out", help="HTML file (default DIR/report.html)")
    plot.add_argument("--cdn", action="store_true", help="load plotly.js from the CDN")
    return p


def _sets(items):
    yaml = {}
    for item in items:
        key, sep, value = item.partition("=")
        if not sep:
            raise ValueError(f"--set {item}: expected KEY=VALUE")
        yaml[key] = value
    return yaml


def _overrides(a):
    pairs = (("path", a.data), ("dir", a.out), ("ticker", a.ticker), ("start", a.start))
    pairs += (("end", a.end), ("interval", a.interval))
    plain = {k: v for k, v in pairs if v is not None}
    if a.quiet:
        plain["level"] = "error"
    if a.verbose:
        plain |= {"level": "debug", "log_every": 1}
    yaml = {k: v for k, v in (("column", a.column), ("seed", a.seed)) if v is not None}
    return plain, yaml | _sets(a.set)


def _summary(res):
    rows = [f"{'#':>2} {'support':>7} {'cohesion':>9} {'mean_len':>9} {'stretch':>8}  first start"]
    for i, m in enumerate(res.motifs, 1):
        rows.append(
            f"{i:>2} {m.support:>7} {m.cohesion:>9.4f} {m.mean_len:>9.1f} {m.stretch:>8.2f}"
            f"  {m.occurrences[0][0]}"
        )
    rows.append(f"stop: {res.stop} after {res.generations} generations ({res.elapsed_s:.1f} s)")
    return "\n".join(rows)


def run(a):
    try:
        cfg = _core.build_config(a.config, None, *_overrides(a))
    except ValueError as e:
        print(f"error: {e}", file=sys.stderr)
        return 2
    except OSError as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    cfg["output"]["dir"] = cfg["output"]["dir"] or DEFAULT_OUT
    try:
        values, stamps = market(cfg)
        res = Result(_core.discover(values, cfg, stamps))
    except (ValueError, OSError, RuntimeError) as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    if a.plot:
        res.plot(path=Path(cfg["output"]["dir"]) / "report.html", cdn=a.cdn)
    print(_summary(res))
    return 130 if res.stop == "cancelled" else 0


def _fmt(mean, std, spec):
    return "-" if mean is None else f"{mean:{spec}} ± {std:{spec}}"


def _bench_table(b):
    rows = [f"{'method':<12} {'dataset':<14} {'F1':>13} {'time s':>9} {'rank':>5} failures"]
    for r in b.summary["by_method_dataset"]:
        rank = b.summary["rank"].get(r["label"], {}).get(r["dataset"])
        t = "-" if r["time_s_mean"] is None else f"{r['time_s_mean']:.2f}"
        rank = "-" if rank is None else f"{rank:.2f}"
        rows.append(
            f"{r['label']:<12} {r['dataset']:<14} {_fmt(r['f1_mean'], r['f1_std'], '.3f'):>13}"
            f" {t:>9} {rank:>5} {r['failures']}"
        )
    rows.append(f"best (mean F1): {b.summary['best']}")
    return "\n".join(rows)


def bench(a):
    plain = {} if a.jobs is None else {"jobs": a.jobs}
    try:
        cfg = _core.build_bench_config(a.config, None, plain, _sets(a.set))
    except ValueError as e:
        print(f"error: {e}", file=sys.stderr)
        return 2
    except OSError as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    out = Path(a.out or DEFAULT_BENCH)
    out.mkdir(parents=True, exist_ok=True)
    try:
        b = run_bench(cfg, out, None if a.quiet else sys.stderr)
    except ValueError as e:
        print(f"error: {e}", file=sys.stderr)
        return 2
    except (RuntimeError, OSError) as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    if a.plot:
        b.plot(path=out / "report.html", cdn=a.cdn)
    print(_bench_table(b))
    return 0


def plot(a):
    out = a.out or Path(a.dir) / "report.html"
    try:
        if (Path(a.dir) / "runs.csv").exists() and not (Path(a.dir) / "results.json").exists():
            load_bench(a.dir).plot(path=out, cdn=a.cdn)
            return 0
        res = load(a.dir)
    except (ValueError, OSError) as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    res.plot(a.kind, path=out, cdn=a.cdn)
    return 0


def main(argv=None):
    a = _parser().parse_args(argv)
    try:
        return {"run": run, "bench": bench, "plot": plot}[a.cmd](a)
    except KeyboardInterrupt:
        return 130


if __name__ == "__main__":
    sys.exit(main())
