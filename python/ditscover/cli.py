import argparse
import sys
from pathlib import Path

from . import Result, _core, load

DEFAULT_OUT = "ditscover-out"


def _parser():
    p = argparse.ArgumentParser(prog="ditscover")
    sub = p.add_subparsers(dest="cmd", required=True)
    run = sub.add_parser("run", help="discover motifs and save the results")
    run.add_argument("config", nargs="?", help="YAML configuration")
    run.add_argument("--data", help="CSV/TSV file (data.path)")
    run.add_argument("--column", help="column name or index (data.column)")
    run.add_argument("--seed", help="random seed")
    run.add_argument("--out", help=f"output directory (default ./{DEFAULT_OUT})")
    run.add_argument("--set", action="append", default=[], metavar="KEY=VALUE")
    run.add_argument("--plot", action="store_true", help="also write report.html")
    run.add_argument("--cdn", action="store_true", help="load plotly.js from the CDN")
    level = run.add_mutually_exclusive_group()
    level.add_argument("-q", "--quiet", action="store_true")
    level.add_argument("-v", "--verbose", action="store_true")
    plot = sub.add_parser("plot", help="write the HTML report of a saved result")
    plot.add_argument("dir")
    plot.add_argument(
        "--kind", default="all", choices=["all", "motifs", "overlay", "evolution", "front"]
    )
    plot.add_argument("--out", help="HTML file (default DIR/report.html)")
    plot.add_argument("--cdn", action="store_true", help="load plotly.js from the CDN")
    return p


def _overrides(a):
    plain = {k: v for k, v in (("path", a.data), ("dir", a.out)) if v is not None}
    if a.quiet:
        plain["level"] = "error"
    if a.verbose:
        plain |= {"level": "debug", "log_every": 1}
    yaml = {k: v for k, v in (("column", a.column), ("seed", a.seed)) if v is not None}
    for item in a.set:
        key, sep, value = item.partition("=")
        if not sep:
            raise ValueError(f"--set {item}: expected KEY=VALUE")
        yaml[key] = value
    return plain, yaml


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
        res = Result(_core.discover(None, cfg))
    except (ValueError, OSError) as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    if a.plot:
        res.plot(path=Path(cfg["output"]["dir"]) / "report.html", cdn=a.cdn)
    print(_summary(res))
    return 130 if res.stop == "cancelled" else 0


def plot(a):
    try:
        res = load(a.dir)
    except (ValueError, OSError) as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    res.plot(a.kind, path=a.out or Path(a.dir) / "report.html", cdn=a.cdn)
    return 0


def main(argv=None):
    a = _parser().parse_args(argv)
    try:
        return {"run": run, "plot": plot}[a.cmd](a)
    except KeyboardInterrupt:
        return 130


if __name__ == "__main__":
    sys.exit(main())
