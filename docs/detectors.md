# Detectors

Reforge emits refactoring signals from threshold checks, Tree-sitter analysis,
git churn, and heuristic drift detectors. Findings are signals for review; they
are not automatic proof that code must be changed.

## File and Directory Signals

- `large_file`: source file line count exceeds `--max-file-lines`.
- `large_directory`: direct source-file count exceeds `--max-dir-files`.
- `debt_marker`: a comment line contains `TODO` or `FIXME`.

Hidden paths are skipped unless `--include-hidden` is set. Generated and
dependency directories are skipped unless `--include-generated` is set. Test
files and directories are scanned by default; `--exclude-tests` removes them
before detector-specific analysis runs.

## Similar Functions

`similar_functions` uses Tree-sitter to extract named functions and methods in
Rust, JavaScript, TypeScript/TSX, Python, Go, Java, C#, Kotlin, PHP, and Ruby.
Function bodies are normalized so identifiers become `ID`, strings become
`STR`, and numbers become `NUM`.

Candidates are grouped only within the same language family and same category
of function or method. Similarity uses length ratio, multiset overlap, and a
longest-common-subsequence check.

Controls:

- `--min-similar-functions`
- `--min-function-tokens`
- `--function-similarity`
- `--include-test-similarity`

Test files are excluded from this detector by default.

## Structural Signals

Structural detectors use Tree-sitter for supported languages.

- `long_function`: function line span exceeds `--max-function-lines`.
- `complex_function`: estimated complexity exceeds
  `--max-function-complexity`.
- `deep_nesting`: nested control-flow depth exceeds `--max-nesting-depth`.
- `many_parameters`: parameter count exceeds `--max-function-parameters`.
- `large_type`: type line span or member count exceeds `--max-type-lines` or
  `--max-type-members`.
- `large_public_surface`: public/exported item count exceeds
  `--max-public-items`.
- `import_heavy_file`: import count exceeds `--max-imports`.
- `function_proliferation`: a production file has many functions, high
  functions-per-100-lines density, and a high percentage of small simple
  functions. This is an over-splitting signal, not proof that any function is
  unused.

Tests are excluded from general structural findings unless
`--include-test-structure` is passed.

For Rust, `large_public_surface` counts items with visibility modifiers such as
`pub`, `pub(crate)`, and `pub(super)`, including public re-exports, because each
one expands the module surface visible to another scope.

## Unused Functions

`unused_function` builds a conservative project-wide identifier index for
Rust, JavaScript, TypeScript/TSX, Python, and Go. It reports private named free
functions that have no same-name references outside their own function body.
Java, C#, Kotlin, PHP, and Ruby are parsed for structural and similarity
signals, but skipped for unused-function candidates until their reference and
visibility rules can be modeled more precisely.

The detector skips public or exported functions, methods, common entry-point
names such as `main` and `init`, and test helper definitions by default.
References from scanned test files still count, so production helpers called
only by tests are not reported unless tests are excluded from the scan.

## Dependency Graph Signals

`dependency_cycle` and `dependency_hub` use a conservative source-file import
graph. The detector resolves only imports that point to another scanned source
file under the scan root, such as relative JavaScript/TypeScript imports,
Rust `mod` declarations, Python relative imports, Ruby `require_relative`
calls, and quoted C/C++ includes that resolve to scanned source files.

- `dependency_cycle`: a resolved strongly connected component spans multiple
  source files.
- `dependency_hub`: a project with enough resolved graph data has a file with
  unusually high fan-in or fan-out.

External packages, unresolved aliases, generated paths skipped by scan filters,
and ambiguous language-specific module systems are ignored rather than guessed.

## Duplication and Test-Risk Signals

- `repeated_literal`: string or numeric literals occur at least
  `--min-repeated-literal-occurrences` times after normalization and filtering.
- `repeated_error_pattern`: repeated catch/except/error handling patterns are
  found across supported languages.
- `data_clump`: repeated parameter groups occur at least
  `--min-data-clump-occurrences` times.
- `test_duplication`: repeated setup, fixture, mock, fake, or before-hook
  patterns occur in tests.
- `happy_path_only_tests`: a test file has at least three test cases with
  assertion evidence but no negative, error, or boundary evidence.

Repeated-literal confidence is lower when the literal appears only in tests or
looks like report/fixture text.

## Drift Signals

- `file_naming_drift`: a directory mixes naming styles such as `snake_case`,
  `kebab-case`, `PascalCase`, `camelCase`, `lowercase`, `dot.separated`, or
  `mixed`.
- `directory_drift`: a directory mixes more concepts than the directory-size
  threshold allows.
- `parallel_implementation`: multiple functions/classes appear to implement
  the same capability.
- `shadowed_abstraction`: helper/common/shared/util abstractions are shadowed
  by similar local helpers.
- `duplicate_type_shape`: multiple type-like shapes share enough fields to
  suggest duplicated data modeling.
- `config_key_drift`: config, route, env, endpoint, token, or similar keys are
  repeated across locations.
- `fixture_factory_drift`: test factory, fixture, mock, fake, or sample
  concepts are repeated across locations.
- `generic_bucket_drift`: generic directories such as `common`, `helpers`,
  `shared`, or `utils` accumulate unrelated concepts.
- `adapter_boundary_bypass`: a boundary module exists, but other files make
  direct HTTP, config, filesystem, or logging calls.
- `stale_compatibility_path`: legacy, deprecated, fallback, shim, polyfill,
  or versioned compatibility paths appear without a clear sunset, owner, or
  migration boundary.

Drift detectors use path and identifier heuristics, so grouped cross-file
findings deserve more weight than isolated info-level findings.

## Documentation Signals

When the scan root looks like a project, Reforge checks for a stable
documentation set.

- `missing_documentation_set`: expected docs are missing under `docs/` or at
  the repository root.
- `missing_user_guide`: user-guide topics such as installation, quick start,
  CLI, configuration, output, and troubleshooting are missing.
- `missing_report_schema_docs`: JSON/YAML fields and compatibility
  expectations are undocumented.
- `missing_metrics_model_docs`: raw metrics, findings, hotspots, priority, or
  confidence are undocumented.
- `missing_architecture_docs`: scan pipeline, detector boundaries, data flow,
  or extension points are undocumented.
- `stale_cli_documentation`: docs mention CLI flags but omit current flags.
- `stale_schema_documentation`: report-schema docs omit current fields.

Expected docs include a docs index, user guide, configuration reference, report
schema, metrics model, detector reference, architecture guide, and contributing
guide.

## Interpreting Detector Output

Prefer findings with high priority, high confidence, cross-file spread, and
clear related locations. Treat low-confidence heuristic findings as prompts for
inspection, not automatic refactor instructions.

## Filtering and Suppression

Finding-kind controls use the snake-case detector names above. `--only` keeps
only selected kinds, `--exclude-detector` removes selected kinds,
`--min-priority` keeps findings at or above the final scored priority, and
`--severity warning` keeps warning and critical findings.

Intentional findings can be suppressed in source comments with
`reforge:ignore`, `reforge:ignore-next-line`, or `reforge:ignore-file`.
Each directive accepts an optional comma-separated kind list followed by a
reason. Long-lived suppressions can also be recorded in `reforge.toml` with
`[[suppressions]]` entries.
