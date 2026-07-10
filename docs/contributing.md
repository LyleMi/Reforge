# Contributing

This project follows the repository guidelines in `AGENTS.md`. Keep changes
small, behavior-focused, and covered by targeted tests.

## Setup

Install Rust 1.85 or newer, then run:

```powershell
cargo build
cargo test
```

For a quick end-to-end smoke test:

```powershell
cargo run -- scan . --progress never
```

For stable machine-readable output:

```powershell
cargo run -- scan . --output json --progress never
```

## Development Workflow

Use `cargo fmt` before review:

```powershell
cargo fmt
```

Run tests:

```powershell
cargo test
```

Run Clippy before larger changes:

```powershell
cargo clippy --all-targets --all-features
```

When report formatting or schema behavior changes, include sample human, HTML,
JSON, YAML, or SARIF output in the pull request description.

## Report App Development

The React report app requires Node.js `^20.19.0` or `>=22.12.0` and npm; CI uses
Node.js 22. Vite 8 is installed from the locked frontend dependencies, so use
the package scripts instead of a global Vite installation:

```powershell
cd web\report-app
npm ci
npm run test
npm run build
```

The build refreshes `assets/report-app.js` and `assets/report-app.css`. Rust
embeds those files in offline HTML reports, so commit both generated assets
with the frontend source change.

## Documentation Site

The documentation site uses mdBook 0.5.4. Install that exact version before
building or serving the site locally:

```powershell
cargo install mdbook --version 0.5.4 --locked
```

On Windows, generate the current self-scan sample and serve the site with:

```powershell
.\scripts\serve-docs.ps1
```

Build static files into `target/docs-site` without starting a server:

```powershell
.\scripts\build-docs.ps1
```

On macOS or Linux, use the matching shell scripts:

```bash
sh scripts/serve-docs.sh
sh scripts/build-docs.sh
```

The published documentation root is
`https://lylemi.github.io/Reforge/`; the generated self-scan is published at
`https://lylemi.github.io/Reforge/sample/`. Repository administrators must set
`Settings > Pages > Build and deployment > Source` to `GitHub Actions` before
the Pages workflow can deploy for the first time. Keep the `github-pages`
environment restricted to the `main` branch; the workflow also enforces that
branch boundary for manual runs.

## Tests

Unit tests live next to the modules they exercise under `#[cfg(test)]` or in
module-specific test files included from the module. There is currently no
separate `tests/` directory.

Add tests for:

- CLI parsing and default values when flags change.
- Config precedence and discovery when configuration changes.
- Scanner exclusions, thresholds, ordering, and report fields.
- Detector behavior, including false-positive guards.
- Output stability for human, HTML, JSON, YAML, and SARIF report changes.

Name tests by behavior, such as `parses_output_format` or
`groups_similar_functions`.

## Style

Use idiomatic Rust formatted by `cargo fmt`. Prefer the existing module split:
`cli`, `scan`, `model`, `detectors`, `scoring`, and `output`.

Use `snake_case` for functions, variables, modules, and test names. Use
`PascalCase` for structs, enums, and traits. Keep CLI flags long,
descriptive, and kebab-case.

Avoid unrelated refactors in behavior changes. If a refactor is needed to make
a feature safe, keep it scoped and covered by tests.

## Report Compatibility

JSON, YAML, and SARIF reports are external interfaces. When fields are added,
removed, or renamed:

- Update `SCAN_REPORT_SCHEMA_VERSION`.
- Update `docs/report-schema.md`.
- Update output tests.
- Mention the compatibility impact in the pull request.

Consumers should rely on stable finding `id`, `priority`, `confidence`,
`priority_factors`, and `rank_explanation`; legacy v4 fields are not emitted.

## Commits and Pull Requests

Use Conventional Commits:

```text
feat(scanner): detect directories with many source files
fix(report): keep JSON output stable
docs: add report schema reference
```

Keep descriptions imperative, lowercase, and without a trailing period. Keep
commits scoped to one behavior change.

Pull requests should describe:

- User-visible effect.
- Validation commands run.
- Related issues.
- Sample human, HTML, JSON, YAML, or SARIF output when report formatting changes.

Do not commit generated outputs, dependency directories, build artifacts, or
local scan artifacts. The checked-in `assets/report-app.js` and
`assets/report-app.css` bundles are the sole generated-output exception because
the Rust HTML renderer embeds them.
