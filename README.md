<p align="center">
  <img src="assets/reforge-logo.png" alt="Reforge logo" width="180">
</p>

# Reforge

<p align="center">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-2024-f74c00?logo=rust&logoColor=white">
  <img alt="MSRV" src="https://img.shields.io/badge/MSRV-1.85-2f855a">
  <img alt="License" src="https://img.shields.io/badge/license-Apache--2.0-blue">
  <img alt="Tests" src="https://img.shields.io/badge/tests-76%20passing-brightgreen">
  <img alt="Output formats" src="https://img.shields.io/badge/output-human%20%7C%20json%20%7C%20yaml-6b46c1">
</p>

Reforge is a Rust CLI for finding refactoring signals in source trees. It scans
code for oversized files, structural complexity, repeated implementation
patterns, similar functions, and agent-written-code drift so you can spot the
areas most likely to benefit from consolidation.

It is designed for quick local audits, CI-friendly reports, and reviewing large
or fast-moving codebases before refactoring work starts.

## Highlights

- Scans Rust, JavaScript, TypeScript/TSX, Python, and Go source files.
- Uses Tree-sitter for structural analysis and similar-function detection.
- Reports human-readable, JSON, or YAML output.
- Skips common generated and dependency directories by default.
- Groups noisy findings such as TODO/FIXME markers and similar functions.
- Includes drift checks for duplicate abstractions, data shapes, config keys,
  fixture factories, generic buckets, and adapter boundary bypasses.

## Quick Start

```powershell
cargo run -- scan .
```

For stable machine-readable output:

```powershell
cargo run -- scan . --output json --progress never
```

To write a report to disk:

```powershell
cargo run -- scan . --output-file reforge-report.json --progress never
```

The output file extension selects JSON or YAML automatically unless `--output`
is set explicitly.

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

## What Reforge Detects

Reforge reports findings as `info`, `warning`, or `critical` priority. The
score is a 0-100 refactoring priority, not a defect probability. It combines
the signal's maintenance impact, threshold intensity, cross-file spread,
detector confidence, and actionability. Priority bands are `info` below 35,
`warning` from 35 through 69, and `critical` from 70 upward.

Core scan signals:

- Large files and directories.
- TODO/FIXME debt markers.
- Similar named functions or methods.
- Long, complex, deeply nested, or parameter-heavy functions.
- Large types and large public/exported surfaces.
- Import-heavy files.
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

Tune similar-function detection:

```powershell
cargo run -- scan . --min-function-tokens 60 --function-similarity 0.85
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

## Sample Output

```text
Reforge scan report
Scanned 15 files in 420 ms; 2 findings; 0 similar function groups.

Summary
  Source files: 15
  Directories: 6
  Function candidates: 93

Signals
  Critical: 0
  Warnings: 2
  Info: 0
  Large files: 2
```

Human output includes a summary, signal counts, grouped findings, and a short
reason for each ranking. JSON and YAML use schema version 3, preserve the
existing finding fields, and add `metrics[].dimension`, `metrics[].normalized`,
`score_breakdown`, and `rank_reason`. Very large similar-function groups
include representative `related_locations` so reports stay bounded.

## CLI Reference

| Option | Default | Purpose |
| --- | --- | --- |
| `--max-file-lines` | `800` | Report files above this line count. |
| `--max-dir-files` | `40` | Report directories above this direct source-file count. |
| `--include-hidden` | `false` | Include hidden files and directories. |
| `--include-generated` | `false` | Include dependency and generated output directories. |
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
| `--min-repeated-literal-occurrences` | `4` | Report repeated literals. |
| `--min-data-clump-occurrences` | `3` | Report repeated parameter groups. |
| `--include-test-structure` | `false` | Include tests in general structural checks. |
| `--output` | inferred | Use `human`, `json`, or `yaml`. |
| `--output-file` | stdout | Write the report to a file. |
| `--progress` | `auto` | Use `auto`, `always`, or `never`. |
| `--color` | `auto` | Use `auto`, `always`, or `never`. |

By default, scans skip common generated and dependency directories such as
`target`, `node_modules`, `dist`, `build`, and `out`.

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

- Broaden Tree-sitter structural support beyond Rust, JavaScript/TypeScript,
  Python, and Go.
- Add richer repository-distribution context for priority scoring.
- Expand drift checks for framework-specific boundaries and generated code.
