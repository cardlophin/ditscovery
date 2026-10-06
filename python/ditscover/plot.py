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
    return _dates(fig.update_layout(title="Series and motif occurrences"), res)


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


def _bars(fig, rows, key, colors, row, legend):
    for label, c in colors.items():
        rs = [r for r in rows if r["label"] == label]
        fig.add_bar(
            x=[r["dataset"] for r in rs],
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
