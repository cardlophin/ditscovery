import csv
import datetime
import hashlib
import json

import numpy as np

EXTRA = "pip install 'ditscover[finance]'"
# Provider limits for intraday history (days back from today).
INTRADAY = {"1m": 7, "2m": 60, "5m": 60, "15m": 60, "30m": 60, "60m": 730, "90m": 60, "1h": 730}


def _cache(req):
    from .bench.datasets import cache_dir

    keys = ("ticker", "start", "end", "period", "interval", "field")
    digest = hashlib.sha256(json.dumps([req[k] for k in keys]).encode()).hexdigest()[:16]
    return cache_dir() / "yfinance" / f"{digest}.csv"


def _check_intraday(req, today):
    days = INTRADAY.get(req["interval"])
    if days is None or req["start"] is None:
        return
    oldest = today - datetime.timedelta(days=days)
    if datetime.date.fromisoformat(req["start"]) < oldest:
        limit = f"the last {days} days (start >= {oldest.isoformat()})"
        raise ValueError(
            f"{req['ticker']}: intraday interval {req['interval']} only covers {limit}"
        )


def _download(req):
    try:
        import yfinance as yf
    except ImportError as e:
        raise RuntimeError(f"market data needs yfinance: {EXTRA}") from e
    kw = {k: req[k] for k in ("start", "end", "period", "interval")}
    df = yf.download(req["ticker"], **kw, auto_adjust=True, progress=False, multi_level_index=False)
    if df is None or df.empty:
        span = req["period"] or f"{req['start']}..{req['end']}"
        raise ValueError(
            f"{req['ticker']}: no data for {span} at {req['interval']} (check the ticker)"
        )
    if req["field"] not in df.columns:
        raise ValueError(f"field `{req['field']}` not in {', '.join(map(str, df.columns))}")
    daily = req["interval"] not in INTRADAY
    stamps = [t.strftime("%Y-%m-%d") if daily else t.isoformat() for t in df.index]
    return df[req["field"]].to_numpy(dtype=float), stamps


def _write(path, x, stamps):
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", newline="") as f:
        csv.writer(f).writerows([("timestamp", "value"), *zip(stamps, x, strict=True)])


def _read(path):
    with open(path, newline="") as f:
        rows = list(csv.DictReader(f))
    return np.array([float(r["value"] or "nan") for r in rows]), [r["timestamp"] for r in rows]


# Returns values, ISO timestamps and the resolved range (start, end, no period) to store in the
# effective configuration; open-ended requests end today, so reruns of that config hit the cache.
def fetch(data):
    today = datetime.date.today()
    req = {k: data[k] for k in ("ticker", "start", "end", "period", "interval", "field")}
    if req["start"] and not req["end"]:
        req["end"] = today.isoformat()
    path = _cache(req)
    if path.exists() and not data["refresh"]:
        x, stamps = _read(path)
    else:
        _check_intraday(req, today)
        x, stamps = _download(req)
        _write(path, x, stamps)
    resolved = {"start": req["start"] or stamps[0][:10], "end": req["end"] or today.isoformat()}
    resolved["period"] = None
    if not _cache(req | resolved).exists():
        _write(_cache(req | resolved), x, stamps)
    return x, stamps, resolved
