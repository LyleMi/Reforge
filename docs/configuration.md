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
enable = [
  "reforge.codebase.large_file",
  "reforge.codebase.long_function",
  "reforge.codebase.dependency_cycle",
  "reforge.codebase.similar_functions",
]
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
can only produce advisory Issues. The CLI starter configuration enables four
preview rules explicitly. Only enforced stable rules produce policy Issues and participate in the default
policy gate. Explicit `--gate-rule` selections also gate preview advisories as
repository acceptance rules; see the [gate guide](user-guide.md#explicit-repository-acceptance-rules).

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

`low_module_cohesion` uses preset-specific minimums:

| Preset | Module functions | Clustered functions |
| --- | ---: | ---: |
| `strict` | 16 | 40% |
| `balanced` | 20 | 50% |
| `relaxed` | 30 | 60% |

Override them with the positive-integer
`codebase.min-module-functions` and the 0–100
`codebase.min-clustered-function-percent`. The same dotted keys work with
`--set`; `config show` materializes preset-derived values and records their
source.

## Large repositories

Source scanning has no file-count or byte budget. Narrow scope through explicit
ownership exclusions, not truncation. Whole-file copies reuse immutable parsed
syntax and source buffers within a run; similarity grouping filters pairs on
demand instead of retaining a quadratic pair matrix. Analysis currently runs
serially. On a 32-core machine, use `CARGO_BUILD_JOBS=4` for builds,
`RUST_TEST_THREADS=4` for tests, and run repository analyses sequentially to leave resources for other work.

Enable `reforge.codebase.duplicate_file` to inspect complete source copies.
`codebase.min-duplicate-file-lines` defaults to 20 for all presets and must be
positive. This is an advisory threshold, not a reading limit.

For projects with fixtures, review tooling code and distributable cases with
explicit scope configurations. Do not silently classify every `cases/` file as
third-party code: many projects keep authored graders and generators there.
Store reports outside scanned source trees and review both findings and coverage
before choosing `--gate-rule` selections. New or changed suppression and ignore
policies invalidate baseline comparability.

### Path exclusions

`scope.ignore-paths` and repeated `--ignore-path` options accept paths and globs
relative to the analysis root (or the parent directory when analyzing one file).
Matching is case-sensitive. A literal path excludes that file or directory and
its descendants; `vendor` does not match `vendor-old` or `src/vendor`.

- `*` and `?` match within one path component: `src/*.py`, `src/test_?.py`.
- `**` matches across directories: `**/*.min.js`, `cases/**`.
- Character classes and alternatives are supported: `src/test_[ab].py`,
  `{cases,audits}/**`.
- Use `**/vendor` to exclude directories named `vendor` at any depth.

Patterns use `/`; backslashes are normalized to `/` for compatibility with
existing path exclusions. Leading/trailing slashes and a leading `./` are
accepted. Patterns do not support gitignore negation (`!`). Invalid glob syntax
fails the scan with the configuration key and offending pattern.

```sh
reforge analyze . --ignore-path 'cases/**' --ignore-path '**/*.min.js'
```

Quote CLI patterns so the shell passes them unchanged. The matcher is compiled
once per traversal, and excluded directories are pruned before their files are
read. The same exclusions apply to Codebase, Dataflow, and comment commands;
normal generated-directory and gitignore filtering still applies.
