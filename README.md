# Reforge

Reforge is a Rust CLI for detecting refactoring signals in codebases.

The initial version scans source files for simple maintainability signals. The
next natural layer is Tree-sitter based parsing for language-aware metrics such
as function length, nesting depth, import fan-in, and repeated structures.

## Usage

```powershell
cargo run -- scan .
cargo run -- scan D:\path\to\project --max-file-lines 600
cargo run -- scan . --include-generated
```

By default, scans skip common dependency and generated output directories such
as `node_modules`, `dist`, `build`, `out`, and `target`.

## Roadmap

- Add Tree-sitter language adapters.
- Extract language-neutral code structure metrics.
- Score refactoring signals by severity and confidence.
- Emit machine-readable JSON reports.
