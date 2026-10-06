from html import escape
from pathlib import Path

from plotly.offline import get_plotlyjs, get_plotlyjs_version

from . import plot

CSS = """
:root{--bg:#fff;--fg:#1f2328;--muted:#656d76;--card:#f6f8fa;--line:#d0d7de;--accent:#0969da}
@media (prefers-color-scheme: dark){:root{--bg:#0d1117;--fg:#e6edf3;--muted:#8d96a0;
--card:#161b22;--line:#30363d;--accent:#4493f8}}
body{margin:0;padding:16px 24px;background:var(--bg);color:var(--fg);
font:14px/1.4 system-ui,-apple-system,sans-serif}
h1{font-size:20px;margin:0 0 12px}
.cards{display:flex;flex-wrap:wrap;gap:8px;margin-bottom:12px}
.card{background:var(--card);border:1px solid var(--line);border-radius:6px;padding:6px 12px}
.card .label{display:block;color:var(--muted);font-size:12px}
.card .value{font-size:18px;font-weight:600}
table{border-collapse:collapse;margin-bottom:12px;font-variant-numeric:tabular-nums}
th,td{padding:3px 10px;border-bottom:1px solid var(--line);text-align:right}
th[data-sort]{cursor:pointer;user-select:none}th[data-sort]:hover{color:var(--accent)}
.t,th[data-sort=text]{text-align:left}
.tabs{display:flex;gap:4px;border-bottom:1px solid var(--line)}
.tab{background:none;border:0;border-bottom:2px solid transparent;color:var(--muted);
padding:8px 14px;font:inherit;cursor:pointer}
.tab.active{color:var(--fg);border-color:var(--accent)}
.panel{display:none;padding-top:8px}.panel.active{display:block}
"""

# Hidden panels have zero width when plotly first draws them: resize on show.
JS = """
const graphs = el => el.querySelectorAll('.plotly-graph-div');
if (matchMedia('(prefers-color-scheme: dark)').matches) graphs(document).forEach(g => {
  const u = {'paper_bgcolor': '#0d1117', 'plot_bgcolor': '#161b22', 'font.color': '#e6edf3'};
  Object.keys(g.layout).filter(k => /^[xy]axis\\d*$/.test(k))
    .forEach(k => u[k + '.gridcolor'] = '#30363d');
  Plotly.relayout(g, u);
  Plotly.restyle(g, {'error_y.color': '#c9d1d9'});
});
document.querySelectorAll('.tab').forEach(b => b.onclick = () => {
  document.querySelectorAll('.tab,.panel').forEach(e => e.classList.remove('active'));
  const panel = document.getElementById('tab-' + b.dataset.tab);
  b.classList.add('active');
  panel.classList.add('active');
  graphs(panel).forEach(g => Plotly.Plots.resize(g));
});
document.querySelectorAll('th[data-sort]').forEach(th => th.onclick = () => {
  const body = th.closest('table').tBodies[0], i = th.cellIndex, asc = th.dataset.dir !== 'asc';
  th.dataset.dir = asc ? 'asc' : 'desc';
  const key = r => r.cells[i].dataset.v;
  const cmp = th.dataset.sort === 'num' ? (a, b) => key(a) - key(b)
    : (a, b) => key(a).localeCompare(key(b));
  [...body.rows].sort((a, b) => asc ? cmp(a, b) : cmp(b, a)).forEach(r => body.appendChild(r));
});
"""


def _plotlyjs(cdn):
    if cdn:
        src = f"https://cdn.plot.ly/plotly-{get_plotlyjs_version()}.min.js"
        return f'<script charset="utf-8" src="{src}"></script>'
    return f"<script>{get_plotlyjs()}</script>"


def card(label, value):
    return (
        f'<div class="card"><span class="label">{escape(label)}</span>'
        f'<span class="value">{escape(str(value))}</span></div>'
    )


# headers: [(name, "num" | "text" | None)]; rows: [[(shown, sort value | None)]]
def table(headers, rows):
    th = "".join(
        f'<th data-sort="{kind}">{escape(h)}</th>' if kind else f"<th>{escape(h)}</th>"
        for h, kind in headers
    )

    def td(shown, v):
        if v is None:
            return f"<td>{escape(str(shown))}</td>"
        cls = ' class="t"' if isinstance(v, str) else ""
        return f'<td{cls} data-v="{escape(str(v))}">{escape(str(shown))}</td>'

    body = "".join("<tr>" + "".join(td(*c) for c in r) + "</tr>" for r in rows)
    return f"<table><thead><tr>{th}</tr></thead><tbody>{body}</tbody></table>"


def _fig(fig):
    opts = dict(full_html=False, include_plotlyjs=False, default_width="100%")
    return fig.to_html(config={"responsive": True}, **opts)


def page(title, cards, body, tabs, path=None, cdn=False):
    nav = "".join(
        f'<button class="tab{" active" if i == 0 else ""}" data-tab="{key}">{escape(name)}</button>'
        for i, (key, name, _) in enumerate(tabs)
    )
    panels = "".join(
        f'<section class="panel{" active" if i == 0 else ""}" id="tab-{key}">'
        + "".join(f if isinstance(f, str) else _fig(f) for f in figs)
        + "</section>"
        for i, (key, _, figs) in enumerate(tabs)
    )
    html = (
        "<!doctype html><html><head><meta charset='utf-8'>"
        "<meta name='viewport' content='width=device-width, initial-scale=1'>"
        f"<title>{escape(title)}</title><style>{CSS}</style>{_plotlyjs(cdn)}</head><body>"
        f'<h1>{escape(title)}</h1><div class="cards">{"".join(cards)}</div>{body}'
        f'<nav class="tabs">{nav}</nav>{panels}<script>{JS}</script></body></html>'
    )
    if path is not None:
        Path(path).write_text(html, encoding="utf-8")
    return html


def _motif_rows(res):
    for i, m in enumerate(res.motifs, 1):
        start = m.occurrences[0][0]
        yield [
            (i, i),
            (m.support, m.support),
            (f"{m.cohesion:.4f}", m.cohesion),
            (f"{m.mean_len:.1f}", m.mean_len),
            (f"{m.stretch:.2f}", m.stretch),
            (plot.label(res, start), start),
        ]


def run_report(res, path=None, cdn=False):
    hv = res.history[-1]["hv"] if res.history else None
    cards = [
        card("Stop reason", res.stop),
        card("Generations", res.generations),
        card("Elapsed", f"{res.elapsed_s:.1f} s"),
        card("Final hypervolume", "-" if hv is None else f"{hv:.4f}"),
        card("Motifs", len(res.motifs)),
        card("Dropped events", res.dropped_events),
    ]
    if res.metrics:
        names = (("Precision", "precision"), ("Recall", "recall"), ("F1", "f1"))
        cards += [card(name, f"{res.metrics[k]:.3f}") for name, k in names]
    cols = ("#", "support", "cohesion", "mean length", "stretch", "first start")
    tabs = [
        ("motifs", "Motifs", [plot.motifs(res), plot.overlay(res)]),
        ("evolution", "Evolution", [plot.evolution(res)]),
        ("front", "Pareto front", [plot.front(res)]),
    ]
    body = table([(c, "num") for c in cols], list(_motif_rows(res)))
    return page("ditscover run", cards, body, tabs, path, cdn)


def _pm(r, key, spec=".3f"):
    m, s = r[f"{key}_mean"], r[f"{key}_std"]
    return ("-", None) if m is None else (f"{m:{spec}} ± {s:{spec}}", m)


def _summary_rows(s):
    for r in s["by_method_dataset"]:
        rank = s["rank"].get(r["label"], {}).get(r["dataset"])
        yield [
            (r["label"], r["label"]),
            (r["dataset"], r["dataset"]),
            (r["n_runs"], r["n_runs"]),
            (r["failures"], r["failures"]),
            *(_pm(r, k) for k in ("precision", "recall", "f1")),
            _pm(r, "time_s", ".2f"),
            _pm(r, "delta_mb", ".1f"),
            ("-", None) if rank is None else (f"{rank:.2f}", rank),
        ]


def bench_report(b, path=None, cdn=False):
    s = b.summary
    failures = sum(r["failures"] for r in s["by_method_dataset"])
    cards = [card("Best method (mean F1)", s["best"] or "-"), card("Runs", len(b.runs))]
    cards.append(card("Failures", failures))
    cols = ["runs", "failures", "precision", "recall", "F1", "time s", "memory MB", "rank"]
    headers = [("method", "text"), ("dataset", "text"), *((c, "num") for c in cols)]
    cost = [plot.cost(s), *([plot.scaling(s)] if s["scaling"] else [])]
    tabs = [
        ("summary", "Summary", [table(headers, list(_summary_rows(s)))]),
        ("quality", "Quality", [plot.quality(s)]),
        ("cost", "Cost", cost),
    ]
    return page("ditscover benchmark", cards, "", tabs, path, cdn)
