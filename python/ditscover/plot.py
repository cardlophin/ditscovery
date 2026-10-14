import numpy as np
import plotly.graph_objects as go
from plotly.colors import qualitative
from plotly.subplots import make_subplots

PALETTE = qualitative.Plotly + qualitative.D3 + qualitative.Set2


def color(i):
    return PALETTE[i % len(PALETTE)]


def label(res, i):
    return res.timestamps[i] if res.timestamps else i


# Dated series keep an index axis (gaps such as weekends are not stretched) labelled with dates.
def _dates(fig, res, ticks=8):
    ts = res.timestamps
    if not ts:
        return fig.update_layout(xaxis_title="index")
    at = np.unique(np.linspace(0, len(ts) - 1, ticks).round().astype(int)).tolist()
    fig.update_traces(hovertemplate="%{text}<br>%{y:.4g}<extra>%{fullData.name}</extra>")
    return fig.update_xaxes(tickmode="array", tickvals=at, ticktext=[ts[i] for i in at])


def motifs(res):
    ts = res.timestamps
    fig = go.Figure(
        go.Scatter(y=res.series, text=ts, mode="lines", name="series", line_color="#b0b0b0")
    )
    for i, m in enumerate(res.motifs):
        xs, ys = [], []
        for s, n in m.occurrences:
            xs += [*range(s, s + n), None]
            ys += [*res.series[s : s + n], None]
        text = [None if x is None else ts[x] for x in xs] if ts else None
        fig.add_scatter(
            x=xs, y=ys, text=text, mode="lines", name=f"motif {i + 1}", line=dict(color=color(i))
        )
    if res.labels is not None:
        _labels_band(fig, res)
    return _dates(fig.update_layout(title="Series and motif occurrences"), res)


# One marker per labelled point, coloured by motif, in a band under the series (grey = none).
def _labels_band(fig, res):
    y = np.asarray(res.series, dtype=float)
    lo, span = np.nanmin(y), (np.nanmax(y) - np.nanmin(y)) or 1.0
    idx = np.arange(len(res.labels))
    colors = ["#d0d7de" if lab < 0 else color(lab) for lab in res.labels]
    fig.add_scatter(
        x=idx, y=np.full(len(idx), lo - 0.08 * span), mode="markers", name="labels",
        marker=dict(color=colors, symbol="square", size=6), hoverinfo="skip",
    )  # fmt: skip


def overlay(res):
    titles = [
        f"motif {i + 1}: k={m.support}, cohesion={m.cohesion:.3f}" for i, m in enumerate(res.motifs)
    ]
    fig = make_subplots(rows=max(len(res.motifs), 1), cols=1, subplot_titles=titles)
    for i, m in enumerate(res.motifs):
        for row in m.aligned:
            fig.add_scatter(
                y=row,
                mode="lines",
                line=dict(color=color(i), width=1),
                opacity=0.5,
                showlegend=False,
                row=i + 1,
                col=1,
            )
        fig.add_scatter(
            y=m.prototype,
            mode="lines",
            line=dict(color="black", width=3),
            name=f"prototype {i + 1}",
            row=i + 1,
            col=1,
        )
    return fig.update_layout(
        title="Aligned occurrences and prototype", height=max(300, 220 * len(res.motifs))
    )


def evolution(res):
    gens = [h["gen"] for h in res.history]
    names = ["support", "cohesion", "mean length"]
    fig = make_subplots(rows=4, cols=1, shared_xaxes=True, subplot_titles=["hypervolume", *names])
    fig.add_scatter(x=gens, y=[h["hv"] for h in res.history], name="hypervolume", row=1, col=1)
    for k, name in enumerate(names):
        for stat, dash in (("best", "solid"), ("mean", "dot")):
            ys = [h[stat][k] for h in res.history]
            fig.add_scatter(
                x=gens,
                y=ys,
                name=f"{stat} {name}",
                line=dict(color=color(k), dash=dash),
                row=k + 2,
                col=1,
            )
    return fig.update_layout(title="Evolution per generation", height=900).update_xaxes(
        title_text="generation", row=4, col=1
    )


def _points(h):
    pts = np.asarray(h["front"], dtype=float).reshape(-1, 3)
    return go.Scatter3d(
        x=pts[:, 0],
        y=pts[:, 1],
        z=pts[:, 2],
        mode="markers",
        marker=dict(size=4, color=pts[:, 1], colorscale="Viridis"),
    )


def front(res):
    snaps = [h for h in res.history if h["front"] is not None]
    frames = [go.Frame(data=[_points(h)], name=str(h["gen"])) for h in snaps]
    fig = go.Figure(data=frames[-1].data if frames else [go.Scatter3d()], frames=frames)
    steps = [
        dict(
            method="animate",
            label=f.name,
            args=[[f.name], dict(mode="immediate", frame=dict(duration=0, redraw=True))],
        )
        for f in frames
    ]
    return fig.update_layout(
        title="Pareto front by generation",
        scene=dict(xaxis_title="support", yaxis_title="cohesion", zaxis_title="mean length"),
        sliders=[dict(active=len(steps) - 1, currentvalue=dict(prefix="generation "), steps=steps)],
    )


VIEWS = {"motifs": motifs, "overlay": overlay, "evolution": evolution, "front": front}


def plotlyjs(cdn):
    return "cdn" if cdn else True


def method_colors(summary):
    labels = dict.fromkeys(r["label"] for r in summary["by_method_dataset"])
    return {label: color(i) for i, label in enumerate(labels)}


def where(r):
    split = r.get("split") or "test"
    return r["dataset"] if split == "test" else f"{r['dataset']} ({split})"


def _bars(fig, rows, key, colors, row, legend):
    for label, c in colors.items():
        rs = [r for r in rows if r["label"] == label]
        fig.add_bar(
            x=[where(r) for r in rs],
            y=[r[f"{key}_mean"] for r in rs],
            error_y=dict(array=[r[f"{key}_std"] for r in rs]),
            name=label,
            marker_color=c,
            legendgroup=label,
            showlegend=legend,
            row=row,
            col=1,
        )


def quality(summary):
    rows = [r for r in summary["by_method_dataset"] if r["f1_mean"] is not None]
    keys = ("precision", "recall", "f1")
    fig = make_subplots(
        rows=3, cols=1, shared_xaxes=True, subplot_titles=["precision", "recall", "F1"]
    )
    for i, k in enumerate(keys):
        _bars(fig, rows, k, method_colors(summary), i + 1, i == 0)
    fig.update_yaxes(range=[0, 1.05])
    title = "Quality per dataset (mean ± std, failed runs count as 0)"
    return fig.update_layout(title=title, barmode="group", height=800)


def cost(summary):
    rows = summary["by_method_dataset"]
    titles = ["time per run (s)", "memory above baseline (MB)"]
    fig = make_subplots(rows=2, cols=1, shared_xaxes=True, subplot_titles=titles)
    for i, k in enumerate(("time_s", "delta_mb")):
        _bars(fig, rows, k, method_colors(summary), i + 1, i == 0)
    fig.update_yaxes(type="log", row=1, col=1)
    return fig.update_layout(
        title="Cost per dataset (successful runs)", barmode="group", height=700
    )


def _iou(a, b):
    inter = max(0, min(a[0] + a[1], b[0] + b[1]) - max(a[0], b[0]))
    return inter / (a[1] + b[1] - inter)


# (segment, GT set, status, IoU) for every found occurrence and every missed GT occurrence under
# the PROM set assignment: a found occurrence is a hit when it overlaps an unmatched GT occurrence
# of its assigned set with IoU > 0.5; an unassigned found set has GT set None.
def assign(found, gt):
    from . import prom

    found = [[tuple(o) for o in occ] for occ in found]
    gt = [[tuple(o) for o in occ] for occ in gt]
    pairs = {}
    if found:
        pairs = {s["found"]: s["gt"] for s in prom(found, gt)["sets"] if s["found"] is not None}
    out, matched = [], set()
    for j, occ in enumerate(found):
        c = pairs.get(j)
        for o in occ:
            free = [t for t in ([] if c is None else gt[c]) if t not in matched]
            best = max(free, key=lambda t: _iou(o, t), default=None)
            iou = 0.0 if best is None else _iou(o, best)
            if iou > 0.5:
                matched.add(best)
            out.append((o, c, "hit" if iou > 0.5 else "false alarm", iou))
    out += [(o, c, "miss", 0.0) for c, occ in enumerate(gt) for o in occ if o not in matched]
    return out


STATUS_DASH = {"truth": "solid", "hit": "solid", "false alarm": "dash", "miss": "dot"}
UNASSIGNED = "#8c959f"


def _lane(fig, entries, y, method, row, shown):
    groups = {}
    for seg, c, status, iou in entries:
        groups.setdefault((c, status), []).append((seg, iou))
    for (c, status), segs in groups.items():
        name = "unassigned" if c is None else f"class {c + 1}"
        line = dict(color=UNASSIGNED if c is None else color(c), width=7, dash=STATUS_DASH[status])
        xs, ys, text = [], [], []
        for (s, n), iou in segs:
            tip = f"{method or 'ground truth'}: {name}"
            tip += f", {status}, IoU {iou:.2f}" if status in ("hit", "false alarm") else ""
            tip += f", {status}" if status == "miss" else ""
            tip += f"<br>[{s}, {s + n})"
            xs, ys, text = xs + [s, s + n, None], ys + [y, y, None], text + [tip, tip, None]
        fig.add_scatter(
            x=xs, y=ys, text=text, mode="lines", name=name, legendgroup=name,
            showlegend=name not in shown, opacity=0.5 if status == "miss" else 1.0,
            line=line, customdata=None if method is None else [method] * len(xs),
            hovertemplate="%{text}<extra></extra>", row=row, col=1,
        )  # fmt: skip
        shown.add(name)


def _status_legend(fig):
    for status in ("hit", "false alarm", "miss"):
        fig.add_scatter(
            x=[None], y=[None], mode="lines", name=status, legendgroup=status,
            line=dict(color="#57606a", width=4, dash=STATUS_DASH[status]),
        )  # fmt: skip


# items: [(series id, x, gt sets, {label: found sets})]; one lane per method under the series,
# each window coloured by the GT class its set was assigned to.
def errors(items, title="Errors per series"):
    fig = make_subplots(rows=len(items), cols=1, subplot_titles=[i[0] for i in items])
    shown = set()
    for row, (_, x, gt, found) in enumerate(items, 1):
        x = np.asarray(x, dtype=float)
        lo, gap = np.nanmin(x), 0.25 * (np.nanmax(x) - np.nanmin(x) or 1.0)
        fig.add_scatter(y=x, mode="lines", line_color="#b0b0b0", showlegend=False, row=row, col=1)
        truth = [(tuple(o), c, "truth", 1.0) for c, occ in enumerate(gt) for o in occ]
        _lane(fig, truth, lo - gap, None, row, shown)
        for lane, (label, sets) in enumerate(found.items(), 2):
            _lane(fig, assign(sets, gt), lo - lane * gap, label, row, shown)
        labels = ["GT", *found]
        ticks = [lo - (i + 1) * gap for i in range(len(labels))]
        fig.update_yaxes(tickvals=ticks, ticktext=labels, row=row, col=1)
    _status_legend(fig)
    return fig.update_layout(title=title, height=max(300, 260 * len(items)))


# ΔF1 per removed component (bars) with its bootstrap CI, one subplot per split.
def ablation(rows):
    splits = list(dict.fromkeys(r["split"] for r in rows))
    fig = make_subplots(rows=len(splits), cols=1, subplot_titles=splits, shared_xaxes=True)
    datasets = list(dict.fromkeys(r["dataset"] for r in rows))
    for row, split in enumerate(splits, 1):
        for i, d in enumerate(datasets):
            rs = [r for r in rows if r["split"] == split and r["dataset"] == d]
            fig.add_bar(
                x=[r["component"] for r in rs], y=[r["delta_f1"] for r in rs], name=d,
                marker_color=color(i), legendgroup=d, showlegend=row == 1,
                error_y=dict(
                    type="data", symmetric=False,
                    array=[r["ci_high"] - r["delta_f1"] for r in rs],
                    arrayminus=[r["delta_f1"] - r["ci_low"] for r in rs],
                ),
                row=row, col=1,
            )  # fmt: skip
    title = "F1 change when each component is removed (negative = the component helps)"
    return fig.update_layout(title=title, barmode="group", height=350 * len(splits))


def scaling(summary):
    titles = ["time (s)", "memory above baseline (MB)"]
    fig = make_subplots(rows=1, cols=2, subplot_titles=titles)
    for label, c in method_colors(summary).items():
        rs = sorted((r for r in summary["scaling"] if r["label"] == label), key=lambda r: r["n"])
        for col, k in enumerate(("time_s", "delta_mb"), 1):
            fig.add_scatter(
                x=[r["n"] for r in rs],
                y=[r[k] for r in rs],
                mode="lines+markers",
                name=label,
                line_color=c,
                marker_color=c,
                legendgroup=label,
                showlegend=col == 1,
                row=1,
                col=col,
            )
    fig.update_xaxes(type="log", title_text="series length")
    fig.update_yaxes(type="log")
    return fig.update_layout(title="Scaling with series length (median over repeats)")
