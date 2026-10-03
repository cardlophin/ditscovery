# ditscover

Rust core + PyO3 library for unsupervised, variable-length motif discovery in univariate time
series with NSGA-II. Governing rules: `.specify/memory/constitution.md`.

## Language

- All code, identifiers, comments, commit messages, specs, plans, tasks and any other
  documentation MUST be written in English.

## Editing files

- Always modify files with the Edit/Write tools.
- Never edit files through shell scripts: no heredocs or stdin code (`python3 - <<EOF`,
  `cat <<EOF > file`), no `sed -i`/`awk` rewrites.
- Use Bash only to build, test, lint and inspect (`cargo`, `uv`, `maturin`, `pytest`, `git`).

## Code style

- Minimal, non-verbose code. No documentation or docstrings; comments only for non-obvious
  invariants.
- No speculative abstractions: traits/enums only at the declared extension points
  (`Loader`, `Distance`, `Step`, stop criteria).
- Tests first (red → green → refactor).
- Gates: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`,
  `ruff check`, `ruff format`, `pytest`.

## Layout

- `crates/core`: algorithm (no PyO3). `crates/py`: bindings. `python/ditscover`: API, CLI, plots.
- Specs live in `specs/<NNN-feature>/`; follow the Spec Kit flow.
