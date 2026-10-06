# Contract: ground-truth file (JSON)

```json
{
  "name": "synthetic-3",
  "series": [0.12, 0.10, ...],
  "sets": [
    [[120, 58], [904, 63], [2210, 49]],
    [[400, 80], [1500, 77]]
  ]
}
```

- Occurrences are `[start, length]` in original indices; occurrences of all sets must not overlap.
- Same format for synthetic suites, cached TSMD-Bench series and user-provided labels.
