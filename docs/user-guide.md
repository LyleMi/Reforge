# User guide

## Installation

Install the release archive for your platform or run `cargo install --path
tools/reforge` from a source checkout.

Run the default Codebase analysis with:

```sh
reforge analyze . --reproducible
```

Dataflow is explicit: use `--analysis dataflow` alone, or repeat
`--analysis codebase --analysis dataflow` for one combined report. Use `--output`
and `--output-file` for human, HTML, JSON, YAML, or SARIF reports. A baseline
must have schema 27, the same producer name, identity scheme, and workspace
identity. Producer versions and unrelated analysis sets may differ. Coverage,
scope, configuration, policy, or rule-semantic changes produce an `unknown`
baseline state instead of claiming that an Issue is new or resolved.

Debug data stays outside the report:

```sh
reforge analyze . --analysis codebase --metrics-output metrics.json
reforge analyze . --analysis dataflow --flow-ir-output flow-ir.json
```

`reforge rules` lists each rule, owning analysis, description, supported languages, default state, and measurements. `reforge init`, `reforge config validate`, and `reforge config show` own configuration.

## Troubleshooting

Use Coverage and its capability limitations when zero Issues are reported.
Regenerate schema 26 reports rather than editing them. If a Dataflow policy is
rejected, verify that its one language matches the source and each sink path and
symbol names exactly one frontend declaration.
