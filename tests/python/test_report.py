import json
import re
import subprocess
import sys

import numpy as np
import pytest

import ditscover as ds
from ditscover import plot, report

CDN_TAG = 'src="https://cdn.plot.ly/plotly-'


@pytest.fixture(scope="module")
def runs(tmp_path_factory):
    x, sets = ds._core.synthetic(3000, [30, 45], 6, 0.2, 0.1, 5)
    gt = tmp_path_factory.mktemp("gt") / "gt.json"
    gt.write_text(json.dumps({"series": x, "sets": sets}))
    kw = dict(min_len=20, max_len=60, population=40, max_generations=15, snapshot_every=5, seed=2)
    return ds.discover(np.asarray(x), **kw), ds.discover(np.asarray(x), ground_truth=gt, **kw)


def cards(html):
    return re.findall(r'<div class="card"><span class="label">([^<]+)</span>', html)


def test_run_report_cards(runs):
    plain, with_gt = runs
    base = [
        "Stop reason",
        "Generations",
        "Elapsed",
        "Final hypervolume",
        "Motifs",
        "Dropped events",
    ]
    assert cards(report.run_report(plain)) == base
    assert cards(report.run_report(with_gt)) == [*base, "Precision", "Recall", "F1"]


def test_motif_table_is_sortable(runs):
    res = runs[0]
    html = report.run_report(res)
    body = html.split("<tbody>")[1].split("</tbody>")[0]
    assert body.count("<tr>") == len(res.motifs)
    for col in ("support", "cohesion", "mean length", "stretch"):
        assert f'<th data-sort="num">{col}</th>' in html
    values = re.findall(r'data-v="([^"]+)"', body)
    assert values and all(float(v) == float(v) for v in values)


def test_labels_band_and_quality_columns(runs):
    res = runs[0]
    fig = plot.motifs(res)
    assert any(t.name == "labels" for t in fig.data)
    html = report.run_report(res)
    for col in ("margin", "extent", "coverage", "quality"):
        assert f'<th data-sort="num">{col}</th>' in html
    body = html.split("<tbody>")[1].split("</tbody>")[0]
    rows = re.findall(r"<tr>(.*?)</tr>", body)
    quality = [float(re.findall(r'data-v="([^"]+)"', r)[-2]) for r in rows]
    assert quality == sorted(quality, reverse=True)


def test_tabs_and_resize(runs):
    html = report.run_report(runs[0])
    tabs = re.findall(r'<button class="tab[^"]*" data-tab="([^"]+)">([^<]+)</button>', html)
    assert [t[1] for t in tabs] == ["Motifs", "Evolution", "Pareto front"]
    for key, _ in tabs:
        assert len(re.findall(f'<section class="panel[^"]*" id="tab-{key}"', html)) == 1
    assert "Plotly.Plots.resize" in html and "prefers-color-scheme: dark" in html


def test_plotly_is_embedded_once_or_from_the_cdn(runs, tmp_path):
    html = report.run_report(runs[0], tmp_path / "r.html")
    assert (tmp_path / "r.html").read_text() == html
    assert html.count("* plotly.js v") == 1 and CDN_TAG not in html
    assert not re.search(r'<(script|link|img)[^>]+(src|href)="https?://', html)
    cdn = report.run_report(runs[0], cdn=True)
    assert cdn.count(CDN_TAG) == 1 and len(cdn) < 1_000_000


def test_result_plot_defaults_to_the_dashboard(runs, tmp_path):
    html = runs[0].plot(path=tmp_path / "r.html")
    assert cards(html) and (tmp_path / "r.html").exists()


def test_dates_on_axes_tooltips_and_table(runs):
    res = runs[0]
    dates = [f"d{i:04d}" for i in range(len(res.series))]
    res.timestamps = dates
    fig = plot.motifs(res)
    assert list(fig.data[0].text) == dates
    assert set(fig.layout.xaxis.ticktext) <= set(dates) and len(fig.layout.xaxis.ticktext) > 2
    first = res.motifs[0].occurrences[0][0]
    assert f'<td data-v="{first}">{dates[first]}</td>' in report.run_report(res)
    res.timestamps = None
    assert plot.motifs(res).data[0].text is None


def _row(label, dataset, series, status, f1, n=1500):
    q = f1 if status == "ok" and dataset != "market" else None
    return dict(
        method=label, label=label, dataset=dataset, series=series, seed=None, cfg_hash="h",
        status=status, reason=None, precision=q, recall=q, f1=q, tp=None, fp=None, fn=None,
        n_sets=2, time_s=0.5 if label == "A" else 2.0, peak_mb=50.0, delta_mb=3.0, threads=1, n=n,
    )  # fmt: skip


@pytest.fixture
def bench(tmp_path):
    from ditscover.bench import _write, summarize

    runs = [
        _row("A", "syn", "syn#0", "ok", 0.8),
        _row("B", "syn", "syn#0", "ok", 0.4),
        _row("A", "syn", "syn#1", "error", None),
        _row("B", "syn", "syn#1", "ok", 0.6),
        _row("A", "market", "market#0", "ok", None),
        _row("B", "market", "market#0", "ok", None),
        *(_row(m, "scaling", f"n={n}#0", "ok", 0.5, n) for m in "AB" for n in (1000, 2000)),
    ]
    summary = summarize(runs, {"market"}) | {"versions": {}, "config": {}}
    (tmp_path / "summary.json").write_text(json.dumps(summary))
    _write(tmp_path / "runs.csv", runs, "w")
    return ds.load_bench(tmp_path)


def test_bench_report_cards_and_summary_table(bench):
    html = report.bench_report(bench)
    assert cards(html) == ["Best method (mean F1)", "Runs", "Failures"]
    values = re.findall(r'<span class="value">([^<]+)</span>', html)
    assert values == ["B", str(len(bench.runs)), "1"]
    body = html.split("<tbody>")[1].split("</tbody>")[0]
    assert body.count("<tr>") == len(bench.summary["by_method_dataset"]) == 4
    assert '<th data-sort="num">failures</th>' in html
    tabs = re.findall(r'<button class="tab[^"]*" data-tab="[^"]+">([^<]+)</button>', html)
    assert tabs == ["Summary", "Quality", "Cost"]


def test_bench_figures_use_one_colour_per_method(bench):
    s = bench.summary
    figs = [plot.quality(s), plot.cost(s), plot.scaling(s)]
    colours = {}
    for fig in figs:
        for t in fig.data:
            assert colours.setdefault(t.name, t.marker.color) == t.marker.color
    assert set(colours) == {"A", "B"}
    assert len(figs[0].data) == 3 * 2
    assert all("market" not in t.x for t in figs[0].data)
    memory = [t for t in figs[1].data if t.yaxis == "y2"]
    assert [list(t.y) for t in memory] == [[3.0, 3.0], [3.0, 3.0]]
    assert {tuple(t.x) for t in figs[2].data} == {(1000, 2000)}


def test_errors_assign_status_class_and_iou():
    gt = [[(0, 10), (50, 10), (100, 10)], [(300, 20), (400, 20)]]
    found = [[(1, 10), (52, 10), (200, 10)], [(600, 5), (700, 5)]]
    by = {}
    for seg, cls, status, iou in plot.assign(found, gt):
        by.setdefault(status, {})[seg] = (cls, round(iou, 3))
    assert by["hit"] == {(1, 10): (0, 0.818), (52, 10): (0, 0.667)}
    assert set(by["false alarm"]) == {(200, 10), (600, 5), (700, 5)}
    assert by["false alarm"][(200, 10)][0] == 0
    assert by["miss"] == {(100, 10): (0, 0.0), (300, 20): (1, 0.0), (400, 20): (1, 0.0)}
    assert all(s == "miss" for _, _, s, _ in plot.assign([], gt))


def test_errors_figure_colours_windows_by_class_with_one_lane_per_method():
    x = np.sin(np.arange(500) / 5)
    gt = [[(0, 10), (50, 10), (100, 10)], [(200, 10), (250, 10)]]
    found = {"A": [[(1, 10), (52, 10)]], "B": [[(300, 10), (400, 10)]]}
    fig = plot.errors([("s#0", x, gt, found)])
    names = {t.name for t in fig.data}
    assert {"hit", "false alarm", "miss", "class 1", "class 2"} <= names
    lanes = {t.customdata[0] for t in fig.data if t.customdata is not None}
    assert lanes == {"A", "B"}
    hits = [t for t in fig.data if t.customdata is not None and t.line.dash == "solid"]
    assert {t.line.color for t in hits} == {plot.color(0)}
    assert any("IoU 0.82" in (s or "") for t in hits for s in t.text)


def test_cli_plot_detects_a_benchmark_folder(bench):
    cmd = [sys.executable, "-m", "ditscover.cli", "plot", str(bench.out)]
    r = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
    assert r.returncode == 0, r.stderr
    assert "Best method (mean F1)" in (bench.out / "report.html").read_text()
