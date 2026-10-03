from dataclasses import dataclass
from pathlib import Path

import numpy as np

from . import _core

__all__ = ["Motif", "Result", "discover", "load"]
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

    def __repr__(self):
        return (
            f"Result(motifs={len(self.motifs)}, stop={self.stop!r}, generations={self.generations})"
        )

    def save(self, dir):
        _core.save_result(self._raw, str(dir))

    def plot(self, kind="all", path=None, cdn=False):
        from . import plot

        if kind == "all":
            return Report(plot.report(self, path, cdn))
        fig = plot.VIEWS[kind](self)
        if path is not None:
            fig.write_html(path, include_plotlyjs=plot.plotlyjs(cdn))
        return fig


class Report(str):
    def _repr_html_(self):
        return str(self)


def load(dir) -> Result:
    return Result(_core.load_result(str(dir)))


def _plain(overrides):
    return {k: str(v) if isinstance(v, Path) else v for k, v in overrides.items()}


def _config(config, overrides, yaml_overrides=None):
    path, base = (str(config), None) if isinstance(config, str | Path) else (None, config)
    return _core.build_config(path, base, _plain(overrides), yaml_overrides)


def discover(data, config=None, **overrides) -> Result:
    values = None
    if isinstance(data, str | Path):
        overrides = {"path": str(data), **overrides}
    else:
        values = np.ascontiguousarray(np.asarray(data, dtype=np.float64).ravel())
    return Result(_core.discover(values, _config(config, overrides)))
