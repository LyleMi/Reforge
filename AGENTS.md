# Repository Guidelines

## Project Structure & Module Organization

Reforge is a Rust 2024 CLI crate for detecting refactoring signals in source trees. Core code lives in `src/`: `main.rs` wires CLI parsing to scanning and reporting, `cli.rs` defines Clap arguments, `scanner.rs` walks projects and builds findings, `similar_functions.rs` contains Tree-sitter based similarity analysis, and `report.rs` renders human and JSON output. Unit tests are colocated in each module under `#[cfg(test)]`; there is currently no separate `tests/` directory. Build output belongs in `target/` and should not be committed.

## Build, Test, and Development Commands

- `cargo build` compiles the CLI and all dependencies.
- `cargo run -- scan .` runs Reforge against the current repository.
- `cargo run -- scan . --output json --progress never` produces stable machine-readable output.
- `cargo test` runs all inline unit tests.
- `cargo fmt` formats Rust code using rustfmt defaults.
- `cargo clippy --all-targets --all-features` checks common Rust issues before review.

## Coding Style & Naming Conventions

Use idiomatic Rust formatted by `cargo fmt`; keep four-space indentation and avoid manual alignment churn. Prefer small modules with clear ownership boundaries matching the existing `cli`, `scanner`, `report`, and `similar_functions` split. Use `snake_case` for functions, variables, modules, and test names; use `PascalCase` for structs, enums, and traits. Keep CLI flags long, descriptive, and kebab-case, for example `--max-file-lines` and `--function-similarity`.

## Testing Guidelines

Add focused unit tests next to the code they exercise. Name tests by behavior, such as `parses_output_format` or `groups_similar_functions`. When changing scanner behavior, include tests for default exclusions, thresholds, ordering, or report fields as appropriate. Run `cargo test` before submitting changes; run `cargo run -- scan . --progress never` for a quick end-to-end smoke test.

## Commit & Pull Request Guidelines

Recent history uses short imperative summaries, sometimes with Conventional Commit prefixes, for example `feat: detect directories with many source files` and `Add structured scan reports and faster similarity checks`. Keep commits scoped to one behavior change. Pull requests should describe the user-visible effect, list validation commands run, link related issues, and include sample human or JSON output when report formatting changes.

## Security & Configuration Tips

Do not commit generated outputs, dependency directories, or local scan artifacts. Preserve the default behavior that skips common generated directories such as `target`, `node_modules`, `dist`, and `build` unless a change explicitly targets generated-file scanning.
