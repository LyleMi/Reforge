# User guide

Run the default Codebase analysis with:

```sh
reforge analyze . --reproducible
```

Dataflow is explicit: use `--analysis dataflow` alone, or repeat
`--analysis codebase --analysis dataflow` for one combined report. Use `--output`
and `--output-file` for human, HTML, JSON, YAML, or SARIF reports. A baseline
must have the same schema, producer, workspace identity, and analysis set.
Coverage degradation prevents a missing Issue from being classified as
resolved.

Debug data stays outside the report:

```sh
reforge analyze . --analysis codebase --metrics-output metrics.json
reforge analyze . --analysis dataflow --flow-ir-output flow-ir.json
```

`reforge rules` lists each rule, owning analysis, description, supported languages, default state, and measurements. `reforge init`, `reforge config validate`, and `reforge config show` own configuration.
