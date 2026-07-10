# Repository Guidelines

## Project Structure & Module Organization

Reforge is a Rust 2024 CLI crate for detecting refactoring signals in source trees. Core code lives in `src/`: `main.rs` wires CLI parsing to scanning and reporting, `cli.rs` defines Clap arguments, `scan/` handles project walking and scan orchestration, `detectors/` owns structural, similarity, dependency, drift, and documentation analysis, `lang/` contains Tree-sitter adapters, `model/` defines report data, `scoring/` ranks findings and hotspots, and `output/` renders human, HTML, JSON, YAML, and SARIF output. Unit tests are colocated under `#[cfg(test)]` or in module-specific test files included from the owning module; there is currently no separate `tests/` directory. Frontend source lives in `web/report-app`. Its generated `assets/report-app.js` and `assets/report-app.css` bundles are intentionally committed because the Rust HTML renderer embeds them. Other build output belongs in `target/` and should not be committed.

## Build, Test, and Development Commands

- `cargo build` compiles the CLI and all dependencies.
- `cargo run -- scan .` runs Reforge against the current repository.
- `cargo run -- scan . --output json --progress never` produces stable machine-readable output.
- `cargo test` runs all inline unit tests.
- `cargo fmt` formats Rust code using rustfmt defaults.
- `cargo clippy --all-targets --all-features` checks common Rust issues before review.

## Coding Style & Naming Conventions

Use idiomatic Rust formatted by `cargo fmt`; keep four-space indentation and avoid manual alignment churn. Prefer small modules with clear ownership boundaries matching the existing `cli`, `scan`, `detectors`, `lang`, `model`, `scoring`, and `output` split. Use `snake_case` for functions, variables, modules, and test names; use `PascalCase` for structs, enums, and traits. Keep CLI flags long, descriptive, and kebab-case, for example `--max-file-lines` and `--function-similarity`.

## Testing Guidelines

Add focused unit tests next to the code they exercise. Name tests by behavior, such as `parses_output_format` or `groups_similar_functions`. When changing scanner behavior, include tests for default exclusions, thresholds, ordering, or report fields as appropriate. Run `cargo test` before submitting changes; after code changes, run `cargo run -- scan . --progress never` and keep the self-scan at `0 findings`.

## Commit & Pull Request Guidelines

Use Conventional Commits for all commit messages, formatted as `<type>(optional-scope): <description>`, for example `feat(scanner): detect directories with many source files` or `fix(report): keep JSON output stable`. Prefer standard types such as `feat`, `fix`, `docs`, `refactor`, `test`, `build`, and `chore`; use an imperative, lowercase description without a trailing period. Keep commits scoped to one behavior change. Pull requests should describe the user-visible effect, list validation commands run, link related issues, and include sample human or JSON output when report formatting changes.

## Security & Configuration Tips

Do not commit generated outputs, dependency directories, or local scan artifacts, except for `assets/report-app.js` and `assets/report-app.css`. Regenerate and commit those two embedded report assets whenever their `web/report-app` source changes. Preserve the default behavior that skips common generated directories such as `target`, `node_modules`, `dist`, and `build` unless a change explicitly targets generated-file scanning.
