# Reforge

Reforge is a Rust CLI for detecting refactoring signals in codebases.

The initial version scans source files for simple maintainability signals and
Tree-sitter based similar-function groups in Rust, JavaScript, TypeScript/TSX,
Python, and Go.

## Usage

```powershell
cargo run -- scan .
cargo run -- scan D:\path\to\project --max-file-lines 600
cargo run -- scan . --max-dir-files 30
cargo run -- scan . --include-generated
cargo run -- scan . --min-function-tokens 60 --function-similarity 0.85
cargo run -- scan . --include-test-similarity
cargo run -- scan . --progress always --color always
cargo run -- scan . --output json --output-file reforge-report.json --progress never
cargo run -- scan . --output yaml --output-file reforge-report.yaml --progress never
```

By default, scans skip common dependency and generated output directories such
as `node_modules`, `dist`, `build`, `out`, and `target`.

Current signals include source files above `--max-file-lines`, directories with
more direct source files than `--max-dir-files`, comment-based TODO/FIXME
markers, and groups of at least `--min-similar-functions` named functions or
methods whose normalized bodies have at least `--min-function-tokens` and meet
`--function-similarity`. Similar-function analysis skips common test file names
and test directories by default; pass `--include-test-similarity` when test
duplication is the intended target. Reforge also reports agent-written-code
drift indicators, including parallel implementations, shadowed abstractions,
duplicate data shapes, config key drift, fixture factory drift, generic bucket
drift, and adapter boundary bypasses.

Use `--output-file <path>` to write the human, JSON, or YAML report to a file
instead of stdout. A `.json` output file selects JSON output, and `.yaml` or
`.yml` selects YAML output, unless `--output` is set explicitly. Existing files
are overwritten.

The default human report includes a summary, signal counts, and grouped
findings. Repeated TODO/FIXME markers in the same file are grouped, and
similar-function and agent-drift groups show only a few representative
locations. Use
`--output json` or `--output yaml` when you need every finding and related
location.

Progress defaults to `auto`, which shows a dynamic percentage on stderr only
when stderr is a terminal. Use `--progress always` or `--progress never` to
override it. Color defaults to `auto`, which colorizes human output only when
stdout is a terminal; use `--color always` or `--color never` to override it.

## Roadmap

- Extract language-neutral code structure metrics.
- Score refactoring signals by severity and confidence.
- Add more machine-readable report fields.
