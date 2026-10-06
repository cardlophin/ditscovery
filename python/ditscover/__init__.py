from dataclasses import dataclass
from pathlib import Path

import numpy as np

from . import _core

__all__ = [
    "Bench",
    "Motif",
    "Result",
    "bench",
    "discover",
    "load",
    "load_bench",
    "prom",
    "read_truth",
    "synthetic_suite",
]
__version__ = _core.__version__


@dataclass(frozen=True)
class Motif:
    occurrences: list[tuple[int, int]]
    prototype: np.ndarray
    aligned: np.ndarray | None
    support: int
    cohesion: float
    mean_len: float
    stretch: float
    objectives: tuple[float, float, float]

    @classmethod
    def _from(cls, d):
        aligned = None if d["aligned"] is None else np.asarray(d["aligned"], dtype=float)
        return cls(
            [tuple(o) for o in d["occurrences"]],
            np.asarray(d["prototype"], dtype=float),
            aligned,
            d["support"],
            d["cohesion"],
            d["mean_len"],
            d["stretch"],
            tuple(d["objectives"]),
        )


class Result:
    def __init__(self, raw):
        self._raw = raw
        self.motifs = [Motif._from(m) for m in raw["motifs"]]
        self.front = [Motif._from(m) for m in raw["front"]]
        self.history = raw["history"]
        self.stop = raw["stop"]
        self.generations = raw["generations"]
        self.elapsed_s = raw["elapsed_s"]
        self.dropped_events = raw["dropped_events"]
        self.config = raw["config"]
        self.version = raw["version"]
        self.series = np.asarray(raw["series"], dtype=float)
        self.metrics = raw.get("metrics")
        self.timestamps = raw.get("timestamps")

    def __repr__(self):
        return (
            f"Result(motifs={len(self.motifs)}, stop={self.stop!r}, generations={self.generations})"
        )

    def save(self, dir):
        _core.save_result(self._raw, str(dir))

    def plot(self, kind="all", path=None, cdn=False):
        from . import plot, report

        if kind == "all":
            return Report(report.run_report(self, path, cdn))
        fig = plot.VIEWS[kind](self)
        if path is not None:
            fig.write_html(path, include_plotlyjs=plot.plotlyjs(cdn))
        return fig


class Report(str):
    def _repr_html_(self):
        return str(self)


def load(dir) -> Result:
    return Result(_core.load_result(str(dir)))


def _sets(x):
    if isinstance(x, Result):
        return [m.occurrences for m in x.motifs]
    return [[(int(s), int(n)) for s, n in occ] for occ in x]


def prom(found, truth, average="micro", off_target="ignore") -> dict:
    return _core.prom(_sets(found), _sets(truth), average, off_target)


def synthetic_suite(
    dir, n_series=20, n=5000, lens=(40, 60, 80), occ=6, stretch=0.3, noise=0.1, seed=0, name=""
) -> list[Path]:
    spec = dict(name=name, n_series=n_series, n=n, lens=list(lens), occ=occ)
    spec |= dict(stretch=stretch, noise=noise, seed=seed)
    return [Path(p) for p in _core.synthetic_suite(str(dir), spec)]


def read_truth(path):
    x, sets = _core.read_truth(str(path))
    return np.asarray(x, dtype=float), sets


def _plain(overrides):
    return {k: str(v) if isinstance(v, Path) else v for k, v in overrides.items()}


def _config(config, overrides, yaml_overrides=None):
    path, base = (str(config), None) if isinstance(config, str | Path) else (None, config)
    return _core.build_config(path, base, _plain(overrides), yaml_overrides)


def market(cfg):
    if not cfg["data"]["ticker"]:
        return None, None
    from . import sources

    values, stamps, resolved = sources.fetch(cfg["data"])
    cfg["data"].update(resolved)
    return np.ascontiguousarray(values), stamps


def discover(data=None, config=None, **overrides) -> Result:
    values = stamps = None
    if isinstance(data, str | Path):
        overrides = {"path": str(data), **overrides}
    elif data is not None:
        values = np.ascontiguousarray(np.asarray(data, dtype=np.float64).ravel())
    cfg = _config(config, overrides)
    if values is None:
        values, stamps = market(cfg)
    return Result(_core.discover(values, cfg, stamps))


# Imported last (the subpackage needs the names above); rebinding `bench` to the function makes
# `ds.bench(...)` callable while `ditscover.bench.*` stays importable from sys.modules.
from .bench import Bench, bench, load_bench  # noqa: E402
