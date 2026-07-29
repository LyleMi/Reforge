# Analyses

Reforge has two core analyses:

- Codebase observes refactoring signals in files, functions, types,
  dependencies, naming, duplication, and repository context.
- Dataflow observes conservative flow paths and can surface enabled relay,
  fan-out, and declared-policy bypass advisories.

`reforge analyze .` runs Codebase only. Dataflow must be selected explicitly.
Repeat `--analysis` to request one combined report:

```sh
reforge analyze . --analysis codebase
reforge analyze . --analysis dataflow
reforge analyze . --analysis codebase --analysis dataflow
```

Combined analysis walks, reads, and parses each source once. Its Issues combine
the results of the isolated analyses; each rule has exactly one owning
analysis. Coverage remains separate under `coverage.codebase` and
`coverage.dataflow`.

All core rules currently begin at `preview` and default off. Selecting an
analysis still records its observations and coverage; a preview rule surfaces
advisories only when its complete rule ID appears in `[rules].enable`.

Codebase raw metrics and the complete Flow IR are debug artifacts, not report
fields. Request them explicitly with `--metrics-output` and
`--flow-ir-output`.
