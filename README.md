<p align="center">
  <img src="assets/reforge-logo.png" alt="Reforge logo" width="180">
</p>

# Reforge

<p align="center">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-2024-f74c00?logo=rust&logoColor=white">
  <img alt="MSRV" src="https://img.shields.io/badge/MSRV-1.85-2f855a">
  <img alt="License" src="https://img.shields.io/badge/license-Apache--2.0-blue">
  <img alt="Tests" src="https://img.shields.io/badge/tests-162%20passing-brightgreen">
  <img alt="Output formats" src="https://img.shields.io/badge/output-human%20%7C%20html%20%7C%20json%20%7C%20yaml%20%7C%20sarif-6b46c1">
</p>

Reforge is a Rust CLI for reporting source-tree quality signals. It collects
raw file, function, type, and optional git churn metrics first, then derives
hotspots and findings from that project-wide model.

It is designed for quick local audits, CI-friendly reports, and reviewing large
or fast-moving codebases before refactoring work starts.

## Highlights

- Scans Rust, JavaScript, TypeScript/TSX, Python, Go, Java, C#, Kotlin, PHP,
  and Ruby source files.
- Uses Tree-sitter for structural analysis and similar-function detection.
- Reports human-readable, HTML, JSON, YAML, or SARIF output with raw metrics,
  percentile summaries, hotspots, and findings.
- Ranks hotspots with `static`, `churn`, or `hybrid` models.
- Collects git churn in repositories by default with graceful fallback outside
  git history.
- Skips common generated, dependency, and git-ignored paths by default.
- Groups noisy findings such as TODO/FIXME markers and similar functions.
- Flags conservative unused private functions with no project references.
- Includes drift checks for duplicate abstractions, data shapes, config keys,
  fixture factories, generic buckets, adapter boundary bypasses, and stale
  compatibility paths.

## Quick Start

```powershell
cargo run -- scan .
```

For stable machine-readable output:

```powershell
cargo run -- scan . --output json --progress never
```

Disable churn collection for deterministic static-only scans:

```powershell
cargo run -- scan . --churn off --hotspot-model static --output json --progress never
```

To write a report to disk:

```powershell
cargo run -- scan . --output-file reforge-report.json --progress never
```

The output file extension selects HTML, JSON, YAML, or SARIF automatically unless
`--output` is set explicitly.

Generate a static visual report:

```powershell
cargo run -- scan . --output-file reforge-report.html --progress never
```

Generate SARIF for CI code-scanning upload:

```powershell
cargo run -- scan . --output sarif --output-file reforge-report.sarif --progress never
```

## Installation

Reforge requires Rust 1.85 or newer.

Build a local debug binary:

```powershell
cargo build
```

Build an optimized binary:

```powershell
cargo build --release
```

Install from this checkout:

```powershell
cargo install --path .
reforge scan D:\path\to\project
```

## Agent Skill

Reforge includes an optional agent skill at `skills/reforge-scan` for agents
that support a skill-folder workflow. The skill teaches an agent how to run
Reforge, choose stable report formats, interpret findings, and recommend
scoped refactors from scan evidence.

Install it for Codex on Windows:

```powershell
.\scripts\install-agent-skill.ps1
```

Install it for Codex on macOS or Linux:

```bash
sh scripts/install-agent-skill.sh
```

Update an existing install by passing `-Force` or `--force`:

```powershell
.\scripts\install-agent-skill.ps1 -Force
```

```bash
sh scripts/install-agent-skill.sh --force
```

For another agent that consumes the same skill folder shape, pass the directory
that contains skill folders:

```powershell
.\scripts\install-agent-skill.ps1 -Agent generic -SkillsDir D:\path\to\agent\skills -Force
```

```bash
sh scripts/install-agent-skill.sh --agent generic --skills-dir ~/.agent/skills --force
```

The scripts install or update the skill only. Add `-InstallCli` or
`--install-cli` when you also want to run `cargo install --path .`.

## Documentation

The full documentation set lives in [docs/](docs/README.md), including the
[user guide](docs/user-guide.md), [configuration reference](docs/configuration.md),
[report schema](docs/report-schema.md), [metrics model](docs/metrics-model.md),
[detector reference](docs/detectors.md), and
[architecture notes](docs/architecture.md).

## What Reforge Detects

Reforge reports quality data in four layers:

- `raw_metrics`: file, function, type, and churn measurements for the scanned
  tree.
- `metrics_summary`: project-level percentile distributions such as LOC,
  complexity, imports, and churn.
- `hotspots`: model-ranked locations with `priority`, `static_risk`, and
  `churn_risk`.
- `findings`: actionable signals derived from raw metrics and pattern
  detectors.

Priority is refactoring priority, not defect probability. Priority bands are
`info` below 35, `warning` from 35 through 69, and `critical` from 70 upward.
Finding priority is calculated from weighted impact, metric intensity,
cross-file spread, churn pressure, actionability, and detector confidence.
Hotspots do not overwrite finding priority; matching function/type churn is a
small ranking signal, while file-level churn is capped for line-level findings.

Core scan signals:

- Large files and directories.
- TODO/FIXME debt markers.
- Similar named functions or methods.
- Long, complex, deeply nested, or parameter-heavy functions.
- Large types and large public/exported surfaces.
- Import-heavy files.
- Private functions with no references outside their own body.
- Repeated literals and repeated error-handling patterns.
- Data clumps and directory concept drift.
- Mixed file naming styles such as `snake_case`, `kebab-case`, `PascalCase`,
  `camelCase`, and `dot.separated`.

Agent-drift signals:

- Parallel implementations.
- Shadowed abstractions.
- Duplicate data/type shapes.
- Config key drift.
- Fixture factory drift.
- Generic bucket drift.
- Adapter boundary bypasses.
- Stale compatibility paths without sunset or migration boundaries.

Test-specific signals:

- Repeated test setup.
- Conservative happy-path-only risk when several test cases have assertions but
  no negative, error, or boundary evidence.

## Examples

Scan the current repository:

```powershell
cargo run -- scan .
```

Scan another project with a stricter file-size threshold:

```powershell
cargo run -- scan D:\path\to\project --max-file-lines 600
```

Include generated and dependency directories:

```powershell
cargo run -- scan . --include-generated
```

Skip additional paths or disable git ignore filtering:

```powershell
cargo run -- scan . --ignore-path vendor --ignore-path generated/snapshots
cargo run -- scan . --no-gitignore
```

Limit report noise to selected finding kinds or priority bands:

```powershell
cargo run -- scan . --only large_file,complex_function --min-priority 35
cargo run -- scan . --exclude-detector debt_marker --severity warning
```

Exclude test files and test directories from the scan:

```powershell
cargo run -- scan . --exclude-tests
```

Tune similar-function detection:

```powershell
cargo run -- scan . --min-function-tokens 60 --function-similarity 0.85
```

Control churn and hotspot ranking:

```powershell
cargo run -- scan . --churn auto --hotspot-model hybrid
cargo run -- scan . --churn on --churn-window-days 90 --churn-max-commit-lines 1000
```

Include test files in similarity or structural checks:

```powershell
cargo run -- scan . --include-test-similarity
cargo run -- scan . --include-test-structure
```

Produce colored human output with progress:

```powershell
cargo run -- scan . --progress always --color always
```

Write YAML:

```powershell
cargo run -- scan . --output yaml --output-file reforge-report.yaml --progress never
```

Write a static HTML report:

```powershell
cargo run -- scan . --output html --output-file reforge-report.html --progress never
```

Write SARIF:

```powershell
cargo run -- scan . --output sarif --output-file reforge-report.sarif --progress never
```

Fail CI on current warning or critical findings:

```powershell
cargo run -- scan . --output json --progress never --fail-on warning
```

Compare against a prior schema 12 baseline and fail only on new or worse
warning/critical findings:

```powershell
cargo run -- scan . --baseline baseline.json --baseline-mode new-or-worse --fail-on warning --output json --progress never
```

Review only new or worse findings in human output while still showing diff
counts:

```powershell
cargo run -- scan . --baseline baseline.json --show new-or-worse --output human --progress never
```

## Sample Output

```text
Reforge scan
15 files  420 ms  model hybrid  churn enabled

Result
  Signals              2  critical 0 | warning 2 | info 0
  Watchlist            1 hotspots
  Similar groups       0

Scan details
  Source files         15
  Directories          6
  Function candidates  93

Signal mix
  large file           2

Findings
  warning  p=58 c=1.00  large file: 1200 lines
            src/report.rs:1
            metrics file_lines=1200/800 lines
            rank high impact, high confidence

Watchlist
  severity pri  target  why
  warning   56  src/report.rs  churn dominates
```

Human output is organized for terminal triage: `Result` separates threshold
signals from the hotspot `Watchlist`, `Signal mix` summarizes finding kinds,
and each finding includes the ranking reason. HTML output renders the same
scan as a static visual report with summary cards, risk distribution, file
heatmap, dependency map, hotspots, similar-function groups, and prioritized
findings. JSON and YAML use schema version 12 and include `summary`,
`metrics_summary`, `raw_metrics`, `dependency_graph`, `hotspots`, and
`findings`. SARIF output targets SARIF 2.1.0
with rules keyed by finding kind and results fingerprinted by Reforge finding
ID. Findings expose stable `id`, `priority`, `confidence`, `priority_factors`,
`rank_explanation`, `metrics`, and `related_locations`; legacy v4 fields
`score`, `score_breakdown`, and `rank_reason` are not emitted. Very large
similar-function groups include representative `related_locations` so reports
stay bounded.

## CI Gates and Baselines

`--fail-on info|warning|critical` turns Reforge into a CI gate. Without a
baseline, the gate evaluates all current findings after writing the requested
report. With `--baseline <PATH>`, Reforge reads a prior schema 12 JSON or YAML
report and matches findings by stable `id`.

`--baseline-mode` controls which current findings are selected for the gate:

- `new`: findings whose IDs are absent from the baseline.
- `new-or-worse`: new findings plus findings whose priority or severity
  increased. This is the default.
- `all`: all current findings.

Human reports include a `Baseline diff` section when `--baseline` is supplied,
with counts for new, worse, same, and resolved findings. Use
`--show new|new-or-worse|all` to choose which current findings are displayed in
the human `Findings` section. The default is `all`, preserving the normal
report view. This display option does not change `--baseline-mode` or
`--fail-on`.

Older reports without stable IDs are rejected as baselines; regenerate the
baseline with the current Reforge before enabling the gate.

## Git Churn

`--churn auto` is the default. In a git repository, Reforge runs `git log`
with `--no-merges`, `--numstat`, and the configured time window. Outside a git
repository, `auto` falls back to the static model and records the reason in
`summary.churn`.

Use `--churn on` when churn is required; scans fail if git history is
unavailable. Use `--churn off` to skip git entirely. Binary numstat rows,
paths outside the scan root, and commits above `--churn-max-commit-lines` are
ignored so mechanical changes do not dominate the hotspot model.

Hotspot models:

- `static`: ranks by size, complexity, coupling, duplication, drift, and test
  risk.
- `churn`: ranks primarily by commits touched, changed lines, authors, and
  weighted churn.
- `hybrid`: default, using `static_risk * 0.65 + churn_risk * 0.35`.

## Configuration

When `--config` is not provided, Reforge looks for `reforge.toml` from the scan
root upward. CLI values override config values; existing threshold defaults use
config values when the CLI option is not changed.

Create, validate, and inspect config without scanning:

```powershell
cargo run -- init
cargo run -- config validate .
cargo run -- config show . --output json
```

`reforge init [PATH]` writes `reforge.toml` into a directory, or writes the
exact file when `[PATH]` ends with `.toml`. Existing files require `--force`.

```toml
max-file-lines = 600
max-function-complexity = 12
max-imports = 25
max-functions-per-file = 40
max-functions-per-100-lines = 12
max-small-function-ratio = 70

churn = "auto"
hotspot-model = "hybrid"
churn-window-days = 180
churn-max-commit-lines = 2000
ignore-paths = ["vendor", "generated/snapshots"]

[[suppressions]]
kind = "large_file"
path = "src/generated.rs"
line = 1
reason = "generated fixture"
```

Suppressions may omit `kind` to match every finding kind, and may omit `line`
to suppress the whole path. Inline comments also suppress specific findings:
`reforge:ignore [kind[,kind...]] reason`,
`reforge:ignore-next-line [kind[,kind...]] reason`, and
`reforge:ignore-file [kind[,kind...]] reason`. Suppressions remove matching
findings from reports and CI gates, but they do not remove hotspot watchlist
entries derived from raw metrics.

## CLI Reference

| Option | Default | Purpose |
| --- | --- | --- |
| `--max-file-lines` | `800` | Report files above this line count. |
| `--max-dir-files` | `40` | Report directories above this direct source-file count. |
| `--include-hidden` | `false` | Include hidden files and directories. |
| `--include-generated` | `false` | Include dependency and generated output directories. |
| `--no-gitignore` | `false` | Do not apply git ignore rules during scanning. |
| `--exclude-tests` | `false` | Exclude test files and test directories from scanning. |
| `--ignore-path` | none | Additional path to skip; can be repeated. |
| `--only` | none | Report only these finding kinds, as `kind[,kind...]`. |
| `--exclude-detector` | none | Exclude these finding kinds, as `kind[,kind...]`. |
| `--min-priority` | none | Report findings whose final priority is at least this 0-100 value. |
| `--severity` | none | Report findings at or above `info`, `warning`, or `critical`. |
| `--min-similar-functions` | `3` | Minimum group size for similar-function findings. |
| `--min-function-tokens` | `80` | Ignore smaller normalized function bodies. |
| `--function-similarity` | `0.85` | Minimum normalized token similarity. |
| `--include-test-similarity` | `false` | Include tests in similar-function analysis. |
| `--max-function-lines` | `80` | Report functions above this line span. |
| `--max-function-complexity` | `15` | Report functions above this estimated complexity. |
| `--max-nesting-depth` | `4` | Report deeply nested functions. |
| `--max-function-parameters` | `5` | Report functions with too many parameters. |
| `--max-type-lines` | `250` | Report large types by line span. |
| `--max-type-members` | `30` | Report large types by member count. |
| `--max-imports` | `35` | Report import-heavy files. |
| `--max-public-items` | `30` | Report large public/exported surfaces. |
| `--max-functions-per-file` | `40` | Report over-splitting risk only when function count and density signals also match. |
| `--max-functions-per-100-lines` | `12` | Report over-splitting risk only when function density also exceeds this threshold. |
| `--max-small-function-ratio` | `70` | Report over-splitting risk only when this percentage of functions are small and simple. |
| `--min-repeated-literal-occurrences` | `12` | Report repeated literals. |
| `--min-data-clump-occurrences` | `4` | Report repeated parameter groups. |
| `--include-test-structure` | `false` | Include tests in general structural checks. |
| `--config` | discovered | Read a `reforge.toml` config file. |
| `--churn` | `auto` | Use `auto`, `on`, or `off` for git churn metrics. |
| `--hotspot-model` | `hybrid` | Use `static`, `churn`, or `hybrid` hotspot ranking. |
| `--churn-window-days` | `180` | Days of git history to include. |
| `--churn-max-commit-lines` | `2000` | Skip commits above this added+deleted line count. |
| `--baseline` | none | Read a prior schema 12 JSON/YAML report for gate comparison. |
| `--baseline-mode` | `new-or-worse` | Gate on `new`, `new-or-worse`, or `all` findings when a baseline is present. |
| `--show` | `all` | Display `new`, `new-or-worse`, or `all` current findings in human baseline reports. |
| `--fail-on` | none | Exit nonzero when selected findings meet `info`, `warning`, or `critical`. |
| `--output` | inferred | Use `human`, `html`, `json`, `yaml`, or `sarif`. |
| `--output-file` | stdout | Write the report to a file. |
| `--progress` | `auto` | Use `auto`, `always`, or `never`. |
| `--color` | `auto` | Use `auto`, `always`, or `never`. |

By default, scans skip common generated and dependency directories such as
`target`, `node_modules`, `dist`, `build`, and `out`, and they also apply git
ignore rules. Test files are scanned by default, though some test-heavy
analysis is opt-in. Use `--exclude-tests` to remove test files and directories
from the scan, and use `--no-gitignore` to scan paths ignored by git.

## Development

```powershell
cargo fmt
cargo test
cargo clippy --all-targets --all-features
cargo run -- scan . --progress never
```

Unit tests live next to the modules they exercise under `#[cfg(test)]`; there
is currently no separate `tests/` directory.

## Roadmap

- Broaden Tree-sitter structural support beyond the current supported
  languages.
- Expand drift checks for framework-specific boundaries and generated code.
