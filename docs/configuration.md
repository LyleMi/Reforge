# Configuration

`reforge.toml` is versioned with `version = 2`. Generate it with `reforge init`.

```toml
version = 2

[analysis]
enabled = ["codebase"]

[scope]
include-hidden = false
include-generated = false
no-gitignore = false
exclude-tests = false
ignore-paths = []

[rules]
enable = []
disable = []
enforce = []

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

Rule arrays require complete IDs. Duplicate, conflicting, and unknown IDs are
errors. `enforce` implies enable and accepts only stable rules. Experimental
rules remain internal observations; preview rules are off unless enabled and
can only produce advisory Issues. Only explicitly enforced stable rules produce
policy Issues or participate in a gate.

Each Dataflow policy is single-language and names exact sink declarations:

```toml
[[dataflow.policies]]
name = "http-client"
language = "typescript"
protected-paths = ["src/domain/**"]
adapter-paths = ["src/adapters/http/**"]
exempt-paths = ["src/bin/**"]

[[dataflow.policies.sinks]]
path = "src/transport.ts"
symbol = "send"
```

A policy is rejected when its language is unsupported or a sink does not match
exactly one public source symbol. Adapter bypass evidence requires a complete
policy and an all-exact, value-preserving witness. Search budgets limit
exploration and are not smell thresholds.

The versioned file is parsed as optional typed fields. Reforge then creates one
complete effective configuration by applying built-in defaults, preset,
configuration file, `--set`, and CLI scope overrides in that order. `reforge
config show` prints every effective leaf together with its source.
