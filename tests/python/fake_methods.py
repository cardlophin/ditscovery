import os
import time

_calls = 0


def fake(x, min_len, max_len, k, seed=None, mode="ok", sleep=0.0, sets=None):
    global _calls
    _calls += 1
    if log := os.environ.get("FAKE_LOG"):
        with open(log, "a") as f:
            f.write(f"{len(x)}\n")
    if mode == "crash":
        raise ValueError("boom")
    if mode == "sleep" or (mode == "slow_first" and _calls == 1):
        time.sleep(sleep)
    return sets if sets is not None else [[(0, min_len), (2 * max_len, min_len)]] * k


METHODS = {"fake": fake, "fake_det": fake}
DETERMINISTIC = {"fake_det"}
