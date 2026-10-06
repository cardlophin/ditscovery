import datetime
import sys
import types

import numpy as np
import pandas as pd
import pytest

import ditscover as ds
from ditscover import sources


def frame(n=300, start="2024-01-01"):
    idx = pd.bdate_range(start, periods=n)
    close = 100 + np.cumsum(np.sin(np.arange(n) / 5))
    cols = {"Open": close, "High": close + 1, "Low": close - 1, "Close": close, "Volume": 1}
    return pd.DataFrame(cols, index=idx)


@pytest.fixture
def yf(monkeypatch, tmp_path):
    calls = []

    def download(ticker, start=None, end=None, period=None, interval="1d", **kw):
        calls.append(dict(ticker=ticker, start=start, end=end, period=period, interval=interval))
        return pd.DataFrame() if ticker == "NOPE" else frame()

    monkeypatch.setitem(sys.modules, "yfinance", types.SimpleNamespace(download=download))
    monkeypatch.setenv("DITSCOVER_CACHE", str(tmp_path))
    return calls


def data(**kw):
    base = dict(ticker="AAPL", start=None, end=None, period=None, interval="1d", field="Close")
    return base | dict(refresh=False) | kw


def test_fetch_returns_values_and_iso_dates(yf):
    x, ts, eff = sources.fetch(data(start="2024-01-01", end="2025-03-01"))
    assert len(x) == len(ts) == 300 and ts[:2] == ["2024-01-01", "2024-01-02"]
    assert yf == [
        dict(ticker="AAPL", start="2024-01-01", end="2025-03-01", period=None, interval="1d")
    ]
    assert eff == {"start": "2024-01-01", "end": "2025-03-01", "period": None}
    np.testing.assert_allclose(x, frame()["Close"].to_numpy())


def test_cache_and_refresh(yf):
    req = data(start="2024-01-01", end="2025-03-01")
    first = sources.fetch(req)
    second = sources.fetch(req)
    assert len(yf) == 1 and second[1] == first[1]
    np.testing.assert_array_equal(second[0], first[0])
    sources.fetch(req | {"refresh": True})
    assert len(yf) == 2


def test_open_ended_requests_are_resolved(yf):
    today = datetime.date.today().isoformat()
    _, _, eff = sources.fetch(data(start="2024-01-01"))
    assert eff == {"start": "2024-01-01", "end": today, "period": None}
    _, ts, eff = sources.fetch(data(period="1y"))
    assert eff == {"start": ts[0], "end": today, "period": None}
    sources.fetch(data(**eff))
    assert len(yf) == 2


def test_errors_before_discovery(yf):
    with pytest.raises(ValueError, match="no data"):
        sources.fetch(data(ticker="NOPE", period="1y"))
    with pytest.raises(ValueError, match="intraday"):
        sources.fetch(data(interval="5m", start="2020-01-01"))
    with pytest.raises(ValueError, match="Adj"):
        sources.fetch(data(period="1y", field="Adj"))
    assert len(yf) == 2


def test_missing_yfinance_names_the_extra(monkeypatch, tmp_path):
    monkeypatch.setitem(sys.modules, "yfinance", None)
    monkeypatch.setenv("DITSCOVER_CACHE", str(tmp_path))
    with pytest.raises(RuntimeError, match=r"ditscover\[finance\]"):
        sources.fetch(data(period="1y"))


def test_discover_on_a_ticker(yf, tmp_path):
    kw = dict(min_len=10, max_len=30, population=20, max_generations=5)
    res = ds.discover(ticker="AAPL", start="2024-01-01", **kw)
    assert res.timestamps[0] == "2024-01-01" and len(res.timestamps) == len(res.series)
    assert res.config["data"]["end"] == datetime.date.today().isoformat()
    res.save(tmp_path / "run")
    assert ds.load(tmp_path / "run").timestamps == res.timestamps


def test_csv_date_column(tmp_path):
    f = frame(200)
    path = tmp_path / "prices.csv"
    f.rename_axis("Date").reset_index().assign(
        Date=lambda d: d.Date.dt.strftime("%Y-%m-%d")
    ).to_csv(path, index=False)
    kw = dict(min_len=10, max_len=30, population=20, max_generations=5)
    res = ds.discover(path, column="Close", time_column="Date", **kw)
    assert res.timestamps[:2] == ["2024-01-01", "2024-01-02"]
    assert ds.discover(path, column="Close", **kw).timestamps is None


@pytest.mark.network
def test_real_download(tmp_path, monkeypatch):
    pytest.importorskip("yfinance")
    monkeypatch.setenv("DITSCOVER_CACHE", str(tmp_path))
    x, ts, _ = sources.fetch(data(ticker="^GSPC", start="2023-01-01", end="2024-01-01"))
    assert len(x) > 200 and ts[0].startswith("2023-01")
