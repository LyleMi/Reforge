# Reforge

Reforge analyzes a repository and returns refactoring evidence with explicit
coverage. A report tells you both what was observed and where analysis was
partial, so an empty issue list is meaningful only alongside coverage.

`analyze → report`

`report = coverage + issues`

`issue = subject + evidence`

`evidence = rule + measurements/locations/witness`

Codebase runs by default. Dataflow is opt-in and reports only complete, exact
paths that meet its conservative thresholds. Select both to produce one
combined report over a shared workspace index.

```sh
cargo build
cargo run -p reforge -- analyze . --reproducible
cargo run -p reforge -- analyze . --analysis dataflow --output json --reproducible
cargo run -p reforge -- analyze . --analysis codebase --analysis dataflow --reproducible
cargo run -p reforge -- rules
```

`reforge analyze` supports human, offline HTML, JSON, YAML, and SARIF output; baselines and `new`/`all` gates; reproducible serialization; and source-scope options. Raw Codebase metrics and the complete Flow IR are debug sidecars written only with `--metrics-output` and `--flow-ir-output`.

Configuration is versioned in `reforge.toml`. Start with `reforge init`, then use `reforge config validate` or `reforge config show`.

See the [user guide](docs/user-guide.md), [configuration reference](docs/configuration.md), [report schema](docs/report-schema.md), [Dataflow contract](docs/dataflow.md), and [0.2 upgrade guide](docs/upgrading-to-0.2.md).

The core product is `reforge analyze` and its report contract.
`reforge-unity` is an experimental specialization backed by the independent
`reforge-unity-engine` and producer `reforge.unity`; `reforge-workflow` is an
optional report consumer for approval-gated agent changes. Neither adds
concepts to the core Codebase/Dataflow model.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```
