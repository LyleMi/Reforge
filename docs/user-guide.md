# User Guide

This guide covers installing Reforge, running scans, choosing output formats,
tuning thresholds, and troubleshooting common problems.

## Installation

Reforge requires Rust 1.85 or newer.

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

## Quick Start

Scan the current repository:

```powershell
cargo run -- scan .
```

Produce stable machine-readable output:

```powershell
cargo run -- scan . --output json --progress never
```

Disable git churn when you want deterministic static-only output:

```powershell
cargo run -- scan . --churn off --hotspot-model static --output json --progress never
```

Write a report to disk:

```powershell
cargo run -- scan . --output-file reforge-report.json --progress never
```

When `--output` is omitted, `--output-file` extensions `.json`, `.yaml`, and
`.yml` select JSON or YAML automatically. Other extensions default to human
output.

## What Gets Scanned

The scanner accepts either a directory or a single file as `[PATH]`. It scans
source files with these extensions:

- Broad source-file discovery: `c`, `cc`, `cpp`, `cs`, `go`, `java`, `js`,
  `jsx`, `kt`, `py`, `rb`, `rs`, `ts`, and `tsx`.
- Tree-sitter structural analysis: Rust, JavaScript, TypeScript/TSX, Python,
  and Go.

By default, hidden files are skipped and common generated or dependency
directories are skipped, including `target`, `node_modules`, `dist`, `build`,
`out`, `coverage`, `.next`, `.nuxt`, `.svelte-kit`, and `.vite`.

Use `--include-hidden` to include hidden paths and `--include-generated` to
include generated or dependency directories.

## Output

Reforge supports `human`, `json`, and `yaml` output.

Human output is intended for terminal review:

```powershell
cargo run -- scan . --output human --progress never
```

JSON and YAML are intended for CI, automation, and agent-to-agent handoff:

```powershell
cargo run -- scan . --output json --progress never
cargo run -- scan . --output yaml --output-file reforge-report.yaml --progress never
```

Use `--progress never` for stable stdout. Progress is written to stderr when
enabled. Use `--color always`, `--color never`, or the default `--color auto`
to control ANSI color in human output.

## Reading Results

Reports contain four main layers:

- `raw_metrics`: file, function, type, and churn measurements.
- `metrics_summary`: percentile summaries for the scanned project.
- `hotspots`: file, function, and type locations ranked by static risk, churn
  risk, or both.
- `findings`: actionable refactoring signals derived from thresholds and
  detectors.

Severity comes from `priority`: `info` is below 35, `warning` is 35 through
69, and `critical` is 70 or above. Priority is a refactoring priority signal,
not a claim that the code is defective.

## Churn and Hotspots

The default `--churn auto` mode collects git churn when the scan root is inside
a git repository. Outside git history, `auto` records the reason and continues
without churn. Use `--churn on` when git churn is required and the scan should
fail if it is unavailable. Use `--churn off` to skip git entirely.

Hotspot models:

- `--hotspot-model static`: rank by static size, complexity, coupling,
  duplication, drift, and test-risk signals.
- `--hotspot-model churn`: rank primarily by commits touched, changed lines,
  authors, and weighted churn.
- `--hotspot-model hybrid`: default ranking, combining static risk at 65% and
  churn risk at 35%.

Tune churn collection with `--churn-window-days` and
`--churn-max-commit-lines`. Commits above the max added+deleted line count are
ignored so large mechanical changes do not dominate results.

## CLI Reference

Usage:

```text
reforge scan [OPTIONS] [PATH]
```

| Option | Default | Purpose |
| --- | --- | --- |
| `--max-file-lines` | `800` | Report files above this total line count. |
| `--max-dir-files` | `40` | Report directories above this direct source-file count. |
| `--include-hidden` | `false` | Include hidden files and directories. |
| `--include-generated` | `false` | Include dependency and generated output directories. |
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
| `--min-repeated-literal-occurrences` | `4` | Report repeated literals seen at least this many times. |
| `--min-data-clump-occurrences` | `3` | Report repeated parameter groups seen at least this many times. |
| `--include-test-structure` | `false` | Include tests in general structural checks. |
| `--config` | discovered | Read a specific configuration file. |
| `--churn` | `auto` | Use `auto`, `on`, or `off` for git churn metrics. |
| `--hotspot-model` | `hybrid` | Use `static`, `churn`, or `hybrid` hotspot ranking. |
| `--churn-window-days` | `180` | Days of git history to include. |
| `--churn-max-commit-lines` | `2000` | Skip commits above this added+deleted line count. |
| `--output` | inferred | Use `human`, `json`, or `yaml`. |
| `--output-file` | stdout | Write the report to a file. |
| `--progress` | `auto` | Use `auto`, `always`, or `never` for progress output. |
| `--color` | `auto` | Use `auto`, `always`, or `never` for human-output color. |
| `--help` | none | Print generated help. |

## Examples

Scan another project with stricter size thresholds:

```powershell
cargo run -- scan D:\path\to\project --max-file-lines 600 --max-function-lines 60
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

Use a specific configuration file:

```powershell
cargo run -- scan . --config reforge.toml --output json --progress never
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
directories.

No similar functions found: lower `--min-function-tokens`, lower
`--function-similarity`, or add `--include-test-similarity` if test code is in
scope.

JSON output is mixed with progress text: add `--progress never`. Progress is
intended for terminals, not machine parsing.
