<p align="center">
  <img src="assets/reforge-logo.png" alt="Reforge logo" width="180">
</p>

# Reforge

<p align="center">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-2024-f74c00?logo=rust&logoColor=white">
  <img alt="MSRV" src="https://img.shields.io/badge/MSRV-1.85-2f855a">
  <img alt="License" src="https://img.shields.io/badge/license-Apache--2.0-blue">
  <img alt="Output formats" src="https://img.shields.io/badge/output-human%20%7C%20html%20%7C%20json%20%7C%20yaml%20%7C%20sarif-6b46c1">
</p>

Reforge is a Rust CLI for reporting source-tree maintainability and
refactoring evidence. It collects directory, file, function, type, dependency,
Unity, and optional git-churn observations before emitting atomic findings,
stable issue decision units, coverage receipts, and agent context/test-
reachability evidence.

It is designed for local audits, CI-friendly reports, and evidence-led review
before refactoring starts. It is not a quality score, health score, priority or
severity model, bug detector, defect probability, or proof that a refactor is
safe.

## Highlights

- Scans Rust, JavaScript, TypeScript/TSX, Vue SFC script blocks, Python, Go,
  Java, C#, Kotlin, PHP, and Ruby with Tree-sitter-backed analysis.
- Reports human, single-file HTML, JSON, YAML, or SARIF output.
- Separates raw observations, project percentiles, atomic `rf3-...` findings,
  and stable `ri3-...` issues.
- Records coverage, detector execution, parse failures, suppressions, and
  unavailable observations so a quiet scan is not mistaken for complete
  evidence.
- Projects dependency closure, unresolved local edges, evidence dispersion,
  and direct/reachable tests through schema 21 `agent_evidence`.
- Collects git churn in repositories by default with graceful fallback when
  history is unavailable.
- Skips common generated, dependency, hidden, and git-ignored paths by default.
- Detects structural pressure, similarity, unused private functions,
  dependency cycles/hubs, repeated patterns, test risk, concept drift,
  documentation drift, agent-written-code drift, and Unity project signals.

## Quick Start

```powershell
cargo run -- scan .
```

Stable machine-readable output:

```powershell
cargo run -- scan . --output json --progress never
```

Reproducible source-only output without git history:

```powershell
cargo run -- scan . --churn off --output json --progress never
```

Write JSON or an offline HTML report:

```powershell
cargo run -- scan . --output-file reforge-report.json --progress never
cargo run -- scan . --output-file reforge-report.html --progress never
```

Recognized output-file extensions select HTML, JSON, YAML, or SARIF unless
`--output` is explicit. Missing parent directories are created automatically.

## Installation

Reforge requires Rust 1.85 or newer. Tagged releases publish platform archives
on the [GitHub Releases page](https://github.com/LyleMi/Reforge/releases).

Build or install from this checkout:

```powershell
cargo build --release
cargo install --path .
reforge scan D:\path\to\project
```

## Agent Skill

The optional `skills/reforge-scan` skill teaches compatible agents to run the
CLI, read schema 21 coverage and evidence, explain limitations, and recommend
scoped follow-up work.

Install the skill and CLI for Codex on Windows:

```batch
.\scripts\install-agent-skill.bat
```

Or use PowerShell directly:

```powershell
.\scripts\install-agent-skill.ps1
```

On macOS or Linux:

```bash
sh scripts/install-agent-skill.sh
```

Pass `-Force` or `--force` to update an existing skill. Pass `-SkipCli` or
`--skip-cli` to install only the skill. A generic skill directory can be
selected explicitly:

```bash
sh scripts/install-agent-skill.sh --agent generic --skills-dir ~/.agent/skills --force
```

The [agent workflow research](docs/agent-workflows.md) compares the current
single skill with a proposed safe, resumable scan/plan/apply/verify workflow.
Only `reforge-scan` is implemented today.

## Documentation

Read the [published documentation](https://lylemi.github.io/Reforge/) or open
the [current self-scan sample](https://lylemi.github.io/Reforge/sample/). Source
documentation includes the [user guide](docs/user-guide.md),
[configuration reference](docs/configuration.md),
[schema 21 report contract](docs/report-schema.md),
[metrics and evidence model](docs/metrics-model.md),
[detector reference](docs/detectors.md),
[HTML report app](docs/report-app.md), and
[architecture notes](docs/architecture.md).

## Report Model

Schema 21 contains these major layers:

- `raw_metrics`, `metrics_summary`, `raw_metric_manifest`, and
  `raw_metric_coverage`: observations and their definitions/availability.
- `dependency_graph`, `agent_evidence`, and `unity_project`: topology and
  project-specific context.
- `coverage_manifest`, `coverage_summary`, and `detector_execution`: what could
  run and what was actually observed.
- `findings`: atomic detector evidence with stable `rf3-...` IDs, metrics,
  recommendations, and related locations.
- `issues`: stable `ri3-...` decision units that group compatible evidence by
  canonical subject and refactor action.
- `suppression_summary`: evidence intentionally removed by source or config
  suppressions.

`findings=0` means no unsuppressed findings remain after detector filters and
suppressions. It does not prove complete coverage, code quality, test
adequacy, refactor safety, or absence of bugs.

## What Reforge Detects

Core signals include:

- large files/directories, TODO/FIXME markers, long/complex/deeply nested or
  parameter-heavy functions, large types/public surfaces, import-heavy files,
  and function proliferation;
- structurally similar functions, repeated literals/error handling/test setup,
  data clumps, and conservative happy-path-only test risk;
- unused private functions, dependency cycles/hubs, naming and directory
  concept drift;
- parallel implementations, shadowed abstractions, duplicate type shapes,
  config/fixture/generic-bucket drift, adapter bypasses, and stale
  compatibility paths;
- stale or missing CLI, schema, user, metrics, and architecture documentation;
- Unity asmdef cycles/hubs/references, GUID/meta/reference integrity,
  serialization/build drift, large scenes/prefabs, serialized-state and
  lifecycle pressure, frame-call risks, Editor/runtime boundaries, and event
  subscription balance.

## Common Commands

```powershell
# Use a stricter built-in threshold set
cargo run -- scan . --preset strict

# Tune individual thresholds
cargo run -- scan . --max-file-lines 600 --max-function-complexity 12

# Select or exclude detector kinds
cargo run -- scan . --only large_file,complex_function
cargo run -- scan . --exclude-detector debt_marker

# Scope files and tests
cargo run -- scan . --include-generated --include-hidden
cargo run -- scan . --exclude-tests
cargo run -- scan . --include-test-similarity --include-test-structure

# Tune similarity
cargo run -- scan . --min-function-tokens 60 --function-similarity 0.9

# Require or disable git churn
cargo run -- scan . --churn on --churn-window-days 90
cargo run -- scan . --churn off

# Require, auto-detect, or disable Unity analysis
cargo run -- scan D:\path\to\unity-project --unity on

# Automation formats
cargo run -- scan . --output yaml --output-file reforge-report.yaml --progress never
cargo run -- scan . --output sarif --output-file reforge-report.sarif --progress never
```

Use `cargo run -- scan --help` for the complete current option set.

## CI Gates and Baselines

Schema 21 gates compare stable finding IDs. A blocking gate requires a current
schema 21 JSON or YAML baseline:

```powershell
cargo run -- scan . --baseline baseline.json --baseline-mode new --fail-on-findings --output json --progress never
```

`--baseline-mode new` selects findings absent from the baseline; `all` selects
every current unsuppressed finding. Human output can show only new evidence:

```powershell
cargo run -- scan . --baseline baseline.json --show new --output human --progress never
```

Reforge writes the requested report before returning a failing exit status.
Older schemas are rejected and must be regenerated. Schema 21 has no
`new-or-worse` mode because it does not assign priority or severity.

## Git Churn

`--churn auto` collects `git log --no-merges --numstat` history when available.
Outside a repository it records an unavailable reason and continues. Use
`--churn on` to require history and `--churn off` to skip it.

The configured time window and maximum added-plus-deleted lines per commit
bound the observation. Binary rows, paths outside the scan root, and oversized
commits are ignored. Churn remains raw context in schema 21; it is not combined
into a hotspot or priority score.

## Configuration

When `--config` is omitted, Reforge discovers `reforge.toml` from the scan root
upward. Threshold precedence is CLI per-threshold value, CLI preset, config
per-threshold value, config preset, then built-in `balanced`.

```powershell
cargo run -- init
cargo run -- config validate .
cargo run -- config show . --output json
```

Example:

```toml
preset = "balanced"
max-file-lines = 600
max-function-complexity = 12
churn = "auto"
churn-window-days = 180
churn-max-commit-lines = 2000
ignore-paths = ["vendor", "generated/snapshots"]

[[suppressions]]
kind = "large_file"
path = "src/generated.rs"
line = 1
reason = "generated fixture"
```

Suppressions remove matching findings but not raw observations. Keep
`suppression_summary` visible when interpreting an empty finding list.

## Development

```powershell
cargo fmt
cargo test
cargo clippy --all-targets --all-features
cargo run -- scan . --progress never
```

Frontend changes under `web/report-app` must regenerate and commit
`assets/report-app.js` and `assets/report-app.css`.

## Roadmap

Near-term work is documented in the
[agent workflow implementation plan](docs/agent-workflows.md): first repair and
test the schema/CLI documentation contract, then prototype a read-only plan
phase before adding optional parallel investigation or source modification.
