# Repository Guidelines

## Project Structure & Module Organization

Reforge is a Rust 2024 workspace for Codebase and Dataflow refactoring analysis. `tools/reforge` owns the core `analyze`, `rules`, `init`, and `config` CLI. `crates/reforge-engine` owns workspace indexing, execution planning, detectors, evidence aggregation, and report projection over the shared schema and output crates. `tools/reforge-unity` is an experimental specialization, while `tools/reforge-workflow` is an optional consumer that governs agent changes from existing reports; neither is part of the core analyzer model. Unit tests are colocated with their owners. Frontend source lives in `web/report-app`; generated `assets/report-app.js` and `assets/report-app.css` are intentionally committed because the HTML renderer embeds them.

## Build, Test, and Development Commands

- `cargo build` compiles the CLI and all dependencies.
- `cargo run -p reforge -- analyze .` runs the default Codebase analysis.
- `cargo run -p reforge -- analyze . --analysis codebase --reproducible` runs Codebase explicitly.
- `cargo run -p reforge -- analyze . --analysis dataflow --output json --reproducible` produces stable Dataflow output.
- `cargo run -p reforge -- analyze . --analysis codebase --analysis dataflow --reproducible` runs both core analyses over one workspace index.
- `cargo test --workspace --all-targets --all-features` runs the complete suite.
- `cargo fmt` formats Rust code using rustfmt defaults.
- `cargo clippy --all-targets --all-features` checks common Rust issues before review.

## Coding Style & Naming Conventions

Use idiomatic Rust formatted by `cargo fmt`; keep four-space indentation and avoid manual alignment churn. Prefer small modules with clear ownership boundaries matching the existing `cli`, `scan`, `detectors`, `lang`, `model`, `evidence_analysis`, and `output` split. Reserve `scan` for source collection, `Codebase` and `Dataflow` for public analyses, `DetectedEvidence` for detector output, and `Issue`/`Evidence` for report types. Use `snake_case` for functions, variables, modules, and test names; use `PascalCase` for structs, enums, and traits. Keep CLI flags long, descriptive, and kebab-case, for example `--max-file-lines` and `--function-similarity`.

## Testing Guidelines

Add focused unit tests next to the code they exercise. Name tests by behavior, such as `parses_output_format` or `groups_similar_functions`. When changing analyzer behavior, include tests for execution isolation, exclusions, thresholds, ordering, witnesses, coverage, or report fields as appropriate. Run the workspace suite before submitting changes; after code changes, run both Codebase and Dataflow self-analysis and keep Codebase at `0` unsuppressed Issues. Disclose Dataflow partial coverage and suppressions.

## Commit & Pull Request Guidelines

Use Conventional Commits for all commit messages, formatted as `<type>(optional-scope): <description>`, for example `feat(codebase): detect directories with many source files` or `fix(report): keep JSON output stable`. Prefer standard types such as `feat`, `fix`, `docs`, `refactor`, `test`, `build`, and `chore`; use an imperative, lowercase description without a trailing period. Keep commits scoped to one behavior change. Pull requests should describe the user-visible effect, list validation commands run, link related issues, and include sample human or JSON output when report formatting changes.

## Security & Configuration Tips

Do not commit generated outputs, dependency directories, or local analysis artifacts, except for `assets/report-app.js` and `assets/report-app.css`. Regenerate and commit those two embedded report assets whenever their `web/report-app` source changes. Preserve the default behavior that skips common generated directories such as `target`, `node_modules`, `dist`, and `build` unless a change explicitly targets generated-file scanning.
