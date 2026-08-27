<p align="center">
  <img src="assets/reforge-logo.png" alt="Reforge" width="144">
</p>

<h1 align="center">Reforge</h1>

<p align="center">
  Find structural drift before it becomes the next refactor.
</p>

<p align="center">
  <a href="https://github.com/LyleMi/Reforge/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/LyleMi/Reforge/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://github.com/LyleMi/Reforge/releases/latest"><img alt="Release" src="https://img.shields.io/github/v/release/LyleMi/Reforge"></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/github/license/LyleMi/Reforge"></a>
  <a href="https://lylemi.github.io/Reforge/"><img alt="Documentation" src="https://img.shields.io/badge/docs-read-1556ad"></a>
</p>

Reforge is a local CLI for repository-level code review. It finds duplicated
implementations, oversized responsibilities, dependency tangles, and
architecture drift—including patterns introduced gradually by coding agents.

Every finding identifies its rule and relevant source locations, with
measurements or a value-flow witness when the rule produces them. Coverage
records what Reforge could and could not analyze. Reforge does not upload source
code, assign a health score, or claim that a finding is a bug.

<p align="center">
  <a href="https://lylemi.github.io/Reforge/playground/"><strong>Try the agent-code Playground →</strong></a>
  &nbsp;·&nbsp;
  <a href="https://lylemi.github.io/Reforge/sample/"><strong>Open an example report →</strong></a>
</p>

## Quick start

Install the latest release on Linux or macOS:

```sh
curl -fsSL https://raw.githubusercontent.com/LyleMi/Reforge/main/scripts/install.sh | sh
```

Or install only the CLI from crates.io:

```sh
cargo install reforge-cli --locked
```

Then analyze a repository:

```sh
reforge analyze .
```

With no configuration, the CLI runs Codebase analysis with four preview
advisories: large files, long functions, dependency cycles, and similar
functions. They are review prompts, not CI failures. Create a versioned starter
configuration to tune or disable them:

```sh
reforge init
reforge analyze . --output html --output-file reforge-report.html
```

Windows PowerShell and pinned-version installation are covered in the
[installation guide](https://lylemi.github.io/Reforge/user-guide.html#install).

## What it finds

| Area | Examples |
| --- | --- |
| Responsibilities | Large files, long or complex functions, deep nesting, broad public surfaces |
| Duplication | Similar functions, repeated literals, repeated test setup, overlapping type shapes |
| Architecture drift | Dependency cycles, generic buckets, parallel implementations, boundary bypasses |
| Repository consistency | Naming drift, stale compatibility paths, TODO/FIXME clusters |

Codebase analysis supports Rust, JavaScript, TypeScript/TSX, Vue, Python, Go,
Java, C#, Kotlin, PHP, Ruby, Bash, and PowerShell. Dependency rules also
recognize C and C++.

## What a finding contains

```text
Implementation duplication: 3 related items

  Rule:        reforge.codebase.shadowed_abstraction
  Locations:   forms/legacy_email_validator.py:1
               forms/signup_email_validator.py:1
               shared/email_validator.py:1
  Measurement: group size 3 (threshold 3)
  Guidance:    Consolidate shared behavior or make separate variants explicit.
```

The evidence makes a finding inspectable; it does not decide the refactor for
you. An empty report is meaningful only for the languages, capabilities, and
rules marked as observed in Coverage.

## Automate review

Export JSON or SARIF for CI and code-scanning integrations:

```sh
reforge analyze . --output sarif --output-file reforge.sarif --reproducible
```

All current core rules are preview and advisory-only. The `--gate` options apply
only to rules that later satisfy Reforge's calibration contract, become stable,
and are explicitly enforced; preview findings do not fail CI.

## Advanced Dataflow analysis

Dataflow is opt-in and intended for exact value-path and declared-boundary
inspection:

```sh
reforge analyze . --analysis dataflow --output json --reproducible
reforge analyze . --analysis codebase --analysis dataflow --reproducible
```

See the [Dataflow guide](docs/dataflow.md) for its configuration and coverage
limits.

## Documentation

- [User guide](https://lylemi.github.io/Reforge/user-guide.html)
- [Codebase analysis](docs/analyses.md)
- [Rule reference](docs/rule-cards.md)
- [Configuration reference](docs/configuration.md)
- [Contributing](docs/contributing.md)

## Development

Reforge is a Rust 2024 workspace. Run the complete validation gate with:

```sh
scripts/check-ci.sh
```

Or run the core checks directly:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets --all-features
```
