from pathlib import Path

import numpy as np
import plotly.graph_objects as go
from plotly.colors import qualitative
from plotly.subplots import make_subplots

PALETTE = qualitative.Plotly + qualitative.D3 + qualitative.Set2


def color(i):
    return PALETTE[i % len(PALETTE)]


def motifs(res):
    fig = go.Figure(go.Scatter(y=res.series, mode="lines", name="series", line_color="#b0b0b0"))
    for i, m in enumerate(res.motifs):
        xs, ys = [], []
        for s, n in m.occurrences:
            xs += [*range(s, s + n), None]
            ys += [*res.series[s : s + n], None]
        fig.add_scatter(x=xs, y=ys, mode="lines", name=f"motif {i + 1}", line=dict(color=color(i)))
    return fig.update_layout(title="Series and motif occurrences", xaxis_title="index")


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


def report(res, path=None, cdn=False):
    parts = [
        view(res).to_html(full_html=False, include_plotlyjs=plotlyjs(cdn) if i == 0 else False)
        for i, view in enumerate(VIEWS.values())
    ]
    html = (
        "<!doctype html><html><head><meta charset='utf-8'><title>ditscover report</title></head>"
        f"<body>{''.join(parts)}</body></html>"
    )
    if path is not None:
        Path(path).write_text(html, encoding="utf-8")
    return html
