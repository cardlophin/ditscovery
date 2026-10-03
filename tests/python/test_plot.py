import subprocess
import sys

import numpy as np
import plotly.graph_objects as go
import pytest

import ditscover as ds
from ditscover import plot


@pytest.fixture(scope="module")
def res():
    x, _ = ds._core.synthetic(3000, [30, 45], 6, 0.2, 0.1, 5)
    return ds.discover(
        x,
        min_len=20,
        max_len=60,
        max_stretch=2.0,
        population=40,
        max_generations=15,
        snapshot_every=5,
        seed=2,
    )


def test_motifs_view(res):
    fig = plot.motifs(res)
    assert len(fig.data) == 1 + len(res.motifs)
    colors = [t.line.color for t in fig.data[1:]]
    assert len(set(colors)) == len(colors)


def test_overlay_uses_core_alignment(res):
    fig = plot.overlay(res)
    assert len(fig.data) == sum(m.support + 1 for m in res.motifs)
    np.testing.assert_allclose(fig.data[0].y, res.motifs[0].aligned[0])


def test_evolution_and_front(res):
    fig = plot.evolution(res)
    assert len(fig.data) == 7
    assert len(fig.data[0].x) == len(res.history)
    snaps = [h for h in res.history if h["front"] is not None]
    fig = plot.front(res)
    assert len(fig.frames) == len(snaps) == 4
    assert len(fig.layout.sliders[0].steps) == len(snaps)


CDN_TAG = 'src="https://cdn.plot.ly/plotly-'


def test_report_is_offline_by_default(res, tmp_path):
    html = res.plot(path=tmp_path / "r.html")
    text = (tmp_path / "r.html").read_text()
    assert text == html and CDN_TAG not in text and len(text) > 1_000_000
    cdn = res.plot(path=tmp_path / "c.html", cdn=True)
    assert CDN_TAG in cdn and len(cdn) < 1_000_000
    assert isinstance(res.plot("motifs"), go.Figure)


def test_cli_plot_without_source_data(res, tmp_path):
    out = tmp_path / "res"
    res.save(out)
    cmd = [sys.executable, "-m", "ditscover.cli", "plot", str(out)]
    r = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
    assert r.returncode == 0, r.stderr
    assert (out / "report.html").stat().st_size > 1_000_000
    r = subprocess.run(
        [*cmd, "--kind", "motifs", "--out", str(tmp_path / "m.html"), "--cdn"],
        capture_output=True,
        text=True,
        timeout=120,
    )
    assert r.returncode == 0 and CDN_TAG in (tmp_path / "m.html").read_text()
