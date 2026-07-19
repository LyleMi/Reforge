# User Guide

This guide covers installing Reforge, running scans, choosing output formats,
tuning thresholds, and troubleshooting common problems.

Reforge reports maintainability and refactoring signals. It does not produce a
quality score, health score, bug probability, or proof that code is safe to
change.

## Installation

Reforge requires Rust 1.85 or newer.

Tagged releases publish platform archives on the
[GitHub Releases page](https://github.com/LyleMi/Reforge/releases). When a
release is available, extract its archive, then move `reforge` or `reforge.exe`
to a directory on `PATH`. The Windows archive is a ZIP; Linux and macOS
archives are compressed tar files. Each archive also contains the README and
license.

Build a debug binary from this checkout:

```powershell
cargo build
```

Build an optimized binary:

```powershell
cargo build --release
```

Install the CLI from this checkout:

```powershell
cargo install --path .
reforge scan D:\path\to\project
```

During local development, the examples below use `cargo run -- scan ...`.
After installation, replace `cargo run -- scan` with `reforge scan`.

## Agent Skill Installation

Reforge ships an optional agent skill in `skills/reforge-scan`. Install it when
you want an agent to run Reforge, choose report formats, interpret findings, or
turn scan output into scoped refactoring recommendations.

For Codex on Windows:

```batch
.\scripts\install-agent-skill.bat
```

Or run the PowerShell script directly:

```powershell
.\scripts\install-agent-skill.ps1
```

For Codex on macOS or Linux:

```bash
sh scripts/install-agent-skill.sh
```

To update an existing install, pass `-Force` or `--force`:

```batch
.\scripts\install-agent-skill.bat --force
```

```bash
sh scripts/install-agent-skill.sh --force
```

For another agent that consumes the same skill folder shape, pass the directory
that contains skill folders:

```batch
.\scripts\install-agent-skill.bat --agent generic --skills-dir D:\path\to\agent\skills --force
```

```bash
sh scripts/install-agent-skill.sh --agent generic --skills-dir ~/.agent/skills --force
```

The install scripts install or update both the skill and the Reforge CLI. They
run `cargo install --path .` from this checkout. Pass `-SkipCli` or
`--skip-cli` to install only the skill.

## Quick Start

Scan the current repository:

```powershell
cargo run -- scan .
```

Produce stable machine-readable output:

```powershell
cargo run -- scan . --output json --progress never
```

Disable git churn when you want deterministic output without repository history:

```powershell
cargo run -- scan . --churn off --output json --progress never
```

Write a report to disk:

```powershell
cargo run -- scan . --output-file reforge-report.json --progress never
```

When `--output` is omitted, `--output-file` extensions `.html`, `.htm`,
`.json`, `.yaml`, `.yml`, and `.sarif` select HTML, JSON, YAML, or SARIF
automatically. Other extensions default to human output. Missing parent
directories in the output path are created automatically, so a path such as
`reports/current/reforge-report.html` does not need to exist before the scan.

## What Gets Scanned

The scanner accepts either a directory or a single file as `[PATH]`. It scans
source files with these extensions:

- Broad source-file discovery: `c`, `cc`, `cjs`, `cpp`, `cs`, `csx`, `cts`,
  `go`, `java`, `js`, `jsx`, `kt`, `mjs`, `mts`, `php`, `py`, `rb`, `rs`,
  `ts`, `tsx`, and `vue`.
- Tree-sitter structural analysis: Rust, JavaScript, TypeScript/TSX, Vue SFC
  script blocks, Python, Go, Java, C#, Kotlin, PHP, and Ruby.

By default, hidden files are skipped and common generated or dependency
directories are skipped, including `target`, `node_modules`, `dist`, `build`,
`out`, `coverage`, `.next`, `.nuxt`, `.svelte-kit`, and `.vite`.
Git ignore rules are also applied by default, including `.gitignore`,
`.git/info/exclude`, and global git ignore files.

Use `--include-hidden` to include hidden paths and `--include-generated` to
include generated or dependency directories. Use `--ignore-path <PATH>` to add
Reforge-specific ignored paths, and use `--no-gitignore` to scan paths ignored
by git.

Test files and test directories such as `tests`, `__tests__`, `spec`, and
`*.test.ts` are scanned by default. Use `--exclude-tests` when you want a
production-source-only scan.

Use finding filters when you want the report to include only selected detector
kinds:

```powershell
cargo run -- scan . --only large_file,complex_function
cargo run -- scan . --exclude-detector debt_marker
```

## Output

Reforge supports `human`, `html`, `json`, `yaml`, and `sarif` output.

Human output is intended for terminal review:

```powershell
cargo run -- scan . --output human --progress never
```

JSON and YAML are intended for CI, automation, and agent-to-agent handoff:

```powershell
cargo run -- scan . --output json --progress never
cargo run -- scan . --output yaml --output-file reforge-report.yaml --progress never
```

SARIF output targets SARIF 2.1.0 for CI code-scanning integrations:

```powershell
cargo run -- scan . --output sarif --output-file reforge-report.sarif --progress never
```

HTML output is a React-powered visual report for local review. It is still
written as a single offline HTML artifact, so it can be opened directly in a
browser without a server:

```powershell
cargo run -- scan . --output html --output-file reforge-report.html --progress never
```

Use `--progress never` for stable stdout. Progress is written to stderr when
enabled. Use `--color always`, `--color never`, or the default `--color auto`
to control ANSI color in human output.

## Reading Results

Human output is organized for quick terminal triage:

- `Result`: total threshold signals, severity counts, hotspot watchlist size,
  suppression summary when present, and similar-function group count.
- `Scan details`: source files, directories, and function candidates scanned.
- `Signal mix`: finding counts by detector kind, shown when findings exist.
- `Findings`: actionable threshold signals sorted by priority.
- `Watchlist`: hotspot locations ranked by static risk, churn risk, or both.

HTML output renders the same report through the React + TypeScript report app
as summary cards, a severity distribution bar, construct/mechanism metadata, the
File Overview, hotspot watchlist, similar-function groups, and prioritized
findings.
When `--output` is omitted, `.html` and `.htm` output-file extensions select
the same HTML report format automatically.

Reports contain four main data layers plus suppression audit context:

- `raw_metrics`: directory, file, function, type, and churn measurements.
- `metrics_summary`: percentile summaries for the scanned project.
- `agent_evidence`: context closure and test reachability evidence for files
  and issues.
- `suppression_summary`: counts of findings removed by suppressions.
- `issues`: compatible atomic evidence grouped into refactoring issues.
- `detector_manifest`: detector coverage, classification, and overlap metadata.
- `findings`: actionable refactoring signals derived from thresholds and
  detectors.

`findings=0` means no findings remain after detector filters and suppressions.
It does not prove code quality, rule out bugs, or mean raw metrics are empty.
When suppressions are used, keep the suppression summary visible so reviewers
can distinguish zero unsuppressed findings from zero observed signals.

Every finding in JSON and YAML has a stable evidence `id` with an `rf3-` prefix.
The ID is derived from the finding kind, primary location, related locations,
and metric names so it can be used for baseline comparison.

Filtering and suppression preserve stable IDs, so baseline comparison remains
consistent whether or not a finding appears in the final report.

`unused_function` findings are conservative dead-code prompts. Reforge reports
private named free functions only when no same-name reference appears outside
the function body. Public/exported functions, methods, and common entry-point
names are skipped.

## Churn

The default `--churn auto` mode collects git churn when the scan root is inside
a git repository. Outside git history, `auto` records the reason and continues
without churn. Use `--churn on` when git churn is required and the scan should
fail if it is unavailable. Use `--churn off` to skip git entirely.

Tune churn collection with `--churn-window-days` and
`--churn-max-commit-lines`. Commits above the max added+deleted line count are
ignored so large mechanical changes do not dominate results.

## CI Gates and Baselines

Use `--fail-on-findings` with `--baseline <PATH>` to make a scan exit nonzero
when unsuppressed evidence IDs are absent from a prior schema 21 JSON or YAML
report. Reforge writes the requested report before returning the failing exit
status. The gate requires a baseline because v21 does not assign severity,
priority, or a readiness score.

`--baseline-mode` controls the selected findings:

- `new`: IDs absent from the baseline.
- `all`: all current findings.

```powershell
cargo run -- scan . --baseline baseline.json --baseline-mode new --fail-on-findings --output json --progress never
```

Human reports include baseline diff counts when `--baseline` is supplied:
`new`, `same`, and `resolved`. Use `--show new|all` to choose which current
findings appear in the human `Findings` section. The default is `all`.

```powershell
cargo run -- scan . --baseline baseline.json --show new --output human --progress never
```

## CLI Reference

Usage:

```text
reforge init [PATH] [--force]
reforge config validate [PATH] [--config CONFIG]
reforge config show [PATH] [--config CONFIG] [--output human|json|yaml]
reforge scan [OPTIONS] [PATH]
```

`init` writes a default `reforge.toml`. `config validate` and `config show`
parse discovered or explicit config without scanning source files or reading
git churn.

| Option | Default | Purpose |
| --- | --- | --- |
| `--preset` | `balanced` | Use `strict`, `balanced`, or `relaxed` threshold defaults before per-threshold overrides. |
| `--unity` | `auto` | Auto-detect Unity roots, require Unity with `on`, or skip Unity with `off`. |
| `--max-unity-assembly-dependencies` | `8` | Assembly dependency threshold. |
| `--max-unity-scene-objects` | `1000` | Serialized scene object threshold. |
| `--max-unity-prefab-objects` | `250` | Serialized prefab object threshold. |
| `--max-unity-serialized-fields` | `16` | Serializable field threshold for Unity types. |
| `--max-unity-lifecycle-methods` | `7` | Unity lifecycle method threshold. |
| `--max-file-lines` | `800` | Report files above this total line count. |
| `--max-dir-files` | `40` | Report directories above this direct source-file count. |
| `--include-hidden` | `false` | Include hidden files and directories. |
| `--include-generated` | `false` | Include dependency and generated output directories. |
| `--no-gitignore` | `false` | Do not apply git ignore rules during scanning. |
| `--exclude-tests` | `false` | Exclude test files and test directories from scanning. |
| `--ignore-path` | none | Additional path to skip; can be repeated. |
| `--only` | none | Report only these finding kinds, as `kind[,kind...]`. |
| `--exclude-detector` | none | Exclude these finding kinds, as `kind[,kind...]`. |
| `--min-similar-functions` | `3` | Report similar-function groups at or above this size. |
| `--min-function-tokens` | `80` | Ignore smaller normalized function bodies. |
| `--function-similarity` | `0.85` | Minimum normalized token similarity for grouping. |
| `--include-test-similarity` | `false` | Include tests in similar-function analysis. |
| `--max-function-lines` | `80` | Report functions above this line span. |
| `--max-function-complexity` | `15` | Report functions above this estimated complexity. |
| `--max-nesting-depth` | `4` | Report functions above this nested control-flow depth. |
| `--max-function-parameters` | `5` | Report functions with more parameters than this threshold. |
| `--max-type-lines` | `250` | Report types above this line span. |
| `--max-type-members` | `30` | Report types above this member count. |
| `--max-imports` | `35` | Report files with more imports than this threshold. |
| `--max-public-items` | `30` | Report files with more public/exported items than this threshold. |
| `--max-functions-per-file` | `40` | Report over-splitting risk only when this function count and density signals are exceeded. |
| `--max-functions-per-100-lines` | `12` | Report over-splitting risk only when function density also exceeds this threshold. |
| `--max-small-function-ratio` | `70` | Report over-splitting risk only when this percentage of functions are small and simple. |
| `--min-repeated-literal-occurrences` | `12` | Report repeated literals seen at least this many times. |
| `--min-data-clump-occurrences` | `4` | Report repeated parameter groups seen at least this many times. |
| `--include-test-structure` | `false` | Include tests in general structural checks. |
| `--config` | discovered | Read a specific configuration file. |
| `--baseline` | none | Read a prior schema 21 JSON/YAML report for gate comparison. |
| `--baseline-mode` | `new` | Select `new` or `all` findings when a baseline is present. |
| `--show` | `all` | Display `new` or `all` current findings in human baseline reports. |
| `--fail-on-findings` | false | Exit nonzero when unsuppressed finding IDs are new relative to the baseline. |
| `--churn` | `auto` | Use `auto`, `on`, or `off` for git churn metrics. |
| `--churn-window-days` | `180` | Days of git history to include. |
| `--churn-max-commit-lines` | `2000` | Skip commits above this added+deleted line count. |
| `--output` | inferred | Use `human`, `html`, `json`, `yaml`, or `sarif`. |
| `--output-file` | stdout | Write the report to a file. |
| `--progress` | `auto` | Use `auto`, `always`, or `never` for progress output. |
| `--color` | `auto` | Use `auto`, `always`, or `never` for human-output color. |
| `--help` | none | Print generated help. |

## Examples

Scan another project with stricter size thresholds:

```powershell
cargo run -- scan D:\path\to\project --max-file-lines 600 --max-function-lines 60
```

Use a built-in threshold preset:

```powershell
cargo run -- scan . --preset strict
```

Tune similar-function detection:

```powershell
cargo run -- scan . --min-similar-functions 4 --min-function-tokens 60 --function-similarity 0.90
```

Include tests in duplication or structural analysis:

```powershell
cargo run -- scan . --include-test-similarity
cargo run -- scan . --include-test-structure
```

Exclude tests entirely:

```powershell
cargo run -- scan . --exclude-tests
```

Use a specific configuration file:

```powershell
cargo run -- scan . --config reforge.toml --output json --progress never
```

Suppress a known intentional finding inline:

```rust
// TODO: generated migration marker reforge:ignore debt_marker tracked in issue 123
// reforge:ignore-next-line large_file generated fixture snapshot
```

## Troubleshooting

`failed to resolve path`: confirm `[PATH]` exists. Put options before or after
the path normally, but do not pass `-- --help` after `scan`; use
`cargo run -- scan --help`.

`scan root is not inside a git repository`: use `--churn off`, or keep
`--churn auto` if churn is optional. Use `--churn on` only when git history is
required.

Unexpected generated files in results: check whether `--include-generated` was
used and whether `ignore-paths` in `reforge.toml` should include local output
directories. If a path is ignored by git and you still want to scan it, add
`--no-gitignore`.

No similar functions found: lower `--min-function-tokens`, lower
`--function-similarity`, or add `--include-test-similarity` if test code is in
scope.

JSON output is mixed with progress text: add `--progress never`. Progress is
intended for terminals, not machine parsing.
