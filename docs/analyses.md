# Analyses

Reforge has two core analyses:

- Codebase finds maintainability signals in files, functions, types,
  dependencies, naming, duplication, documentation, and repository context.
- Dataflow finds strict, witness-backed relay, fan-out, and declared policy
  bypass paths.

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

Codebase raw metrics and the complete Flow IR are debug artifacts, not report
fields. Request them explicitly with `--metrics-output` and
`--flow-ir-output`.
