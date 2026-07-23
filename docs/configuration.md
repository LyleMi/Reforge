# Configuration

`reforge.toml` is versioned with `version = 1`. Generate it with `reforge init`.

```toml
version = 1

[analysis]
enabled = ["codebase"]

[scope]
include-hidden = false
include-generated = false
no-gitignore = false
exclude-tests = false
ignore-paths = []

[codebase]
preset = "balanced"
churn = "auto"
max-file-lines = 600

[dataflow.search]
max-path-steps = 24
max-function-hops = 8
max-module-hops = 8
max-paths-per-source = 100
max-sinks-per-source = 100
work-budget = 100000

[dataflow.relay]
min-function-hops = 4
min-module-hops = 2
min-relay-percent = 90

[dataflow.fan-out]
min-sinks = 4
min-modules = 3
```

`[[dataflow.policies]]` adds explicit adapter policy checks; general Dataflow rules always run whenever Dataflow is selected. Search budgets limit exploration and are not smell thresholds.

The versioned file is parsed as optional typed fields. Reforge then creates one
complete effective configuration by applying built-in defaults, preset,
configuration file, `--set`, and CLI scope overrides in that order. `reforge
config show` prints every effective leaf together with its source.

Reforge 0.2 does not discover or translate `reforge-scan.toml`, `reforge-flow.toml`, or `reforge-unity.toml`.
