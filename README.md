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
```

By default, scans skip common dependency and generated output directories such
as `node_modules`, `dist`, `build`, `out`, and `target`.

Current signals include source files above `--max-file-lines`, directories with
more direct source files than `--max-dir-files`, comment-based TODO/FIXME
markers, and groups of at least `--min-similar-functions` named functions or
methods whose normalized bodies have at least `--min-function-tokens` and meet
`--function-similarity`. Similar-function analysis skips common test file names
and test directories by default; pass `--include-test-similarity` when test
duplication is the intended target.

## Roadmap

- Extract language-neutral code structure metrics.
- Score refactoring signals by severity and confidence.
- Emit machine-readable JSON reports.
