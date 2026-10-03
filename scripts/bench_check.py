import json
import sys
from pathlib import Path

LIMIT = 0.10


def main(root="target/criterion"):
    changes = sorted(Path(root).glob("**/change/estimates.json"))
    if not changes:
        print(f"no criterion comparisons under {root}; run `cargo bench -- --baseline ref` first")
        return 1
    worst = 0.0
    for path in changes:
        bench = path.parent.parent.relative_to(root)
        mean = json.loads(path.read_text())["mean"]["point_estimate"]
        worst = max(worst, mean)
        flag = "REGRESSION" if mean > LIMIT else "ok"
        print(f"{str(bench):40} {mean:+8.2%}  {flag}")
    return 1 if worst > LIMIT else 0


if __name__ == "__main__":
    sys.exit(main(*sys.argv[1:]))
