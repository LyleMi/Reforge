# Detectors

Rules have one owner. Codebase owns source-tree heuristics and repository context; Dataflow owns exact Flow IR paths; Unity owns experimental specialization rules. The rule registry and ownership tests enforce this partition.

## Dataflow analysis

- `reforge.dataflow.excessive_relay` (stable): a complete path meets the configured
  function-hop, module-hop, and relay-percentage thresholds.
- `reforge.dataflow.flow_fan_out` (stable): a complete source reaches more than the configured number of distinct cross-module sink functions.
- `reforge.dataflow.adapter_flow_bypass` (stable policy): an exact protected-to-sink path does not cross a declared adapter.

All require ordered exact witnesses. Budget truncation, dynamic/ambiguous calls and unsupported language behavior become language-specific partial coverage instead of speculative Evidence. Rust, JavaScript/TypeScript/TSX, and Python support the documented local def-use/return/direct-call subset. The heuristic `reforge.codebase.adapter_boundary_bypass` remains a Codebase rule.

## Codebase analysis

Reforge emits refactoring signals from threshold checks, Tree-sitter analysis,
git churn, and heuristic drift detectors. Issues are signals for review; they
are not automatic proof that code must be changed, that code is low quality, or
that a bug exists.

Every rule is described by one internal metadata registry. It declares
analysis ownership, family, subject kind, supported languages, measurements,
description, and guidance. Coverage, `reforge rules`, and suppression
validation consume the same metadata. See the
[metrics model](metrics-model.md).

## File and Directory Signals

- `large_file`: source file line count exceeds `codebase.max-file-lines`.
- `large_directory`: direct source-file count exceeds `codebase.max-dir-files`.
- `debt_marker`: a comment line contains `TODO` or `FIXME`.

Hidden paths are skipped unless `--include-hidden` is set. Generated and
dependency directories are skipped unless `--include-generated` is set. Test
files and directories are scanned by default; `--exclude-tests` removes them
before detector-specific analysis runs.

## Similar Functions

`similar_functions` uses Tree-sitter to extract named functions and methods in
Rust, JavaScript, TypeScript/TSX, Vue SFC script blocks, Python, Go, Java, C#,
Kotlin, PHP, Ruby, Bash, and PowerShell. C# extraction includes methods,
constructors, and local functions.
Function bodies are normalized so identifiers become `ID`, strings become
`STR`, and numbers become `NUM`.

Candidates are grouped only within the same language family and same category
of function or method. Similarity uses length ratio, multiset overlap, and a
longest-common-subsequence check.

Controls:

- `codebase.min-similar-functions`
- `codebase.min-function-tokens`
- `codebase.function-similarity`

Test files are excluded from this detector.

## Structural Signals

Structural detectors use Tree-sitter for supported languages.

- `long_function`: function line span exceeds `codebase.max-function-lines`.
- `complex_function`: estimated complexity exceeds
  `codebase.max-function-complexity`.
- `deep_nesting`: nested control-flow depth exceeds `codebase.max-nesting-depth`.
- `many_parameters`: parameter count exceeds `codebase.max-function-parameters`.
- `large_type`: type line span or member count exceeds `codebase.max-type-lines` or
  `codebase.max-type-members`.
- `large_public_surface`: public/exported item count exceeds
  `codebase.max-public-items`.
- `import_heavy_file`: import count exceeds `codebase.max-imports`.
- `function_proliferation`: a production file has many functions, high
  functions-per-100-lines density, and a high percentage of small simple
  functions. This is an over-splitting signal, not proof that any function is
  unused.

Tests are excluded from general structural Evidence. Use `--exclude-tests` only
when test files should be removed from the source inventory entirely.

For Rust, `large_public_surface` counts items with visibility modifiers such as
`pub`, `pub(crate)`, and `pub(super)`, including public re-exports, because each
one expands the module surface visible to another scope.

## Unused Functions

`unused_function` builds a conservative project-wide identifier index for
Rust, JavaScript, TypeScript/TSX, Vue SFC script blocks, Python, Go, and C#
local functions. It reports private named free functions or C# local functions
that have no same-name references outside their own function body. Java, C#
methods, Kotlin, PHP, Ruby, Bash, and PowerShell are skipped for unused-function
candidates until their reference and visibility rules can be modeled more
precisely.

The detector skips public or exported functions, methods, common entry-point
names such as `main` and `init`, and test helper definitions by default.
References from scanned test files still count, so production helpers called
only by tests are not reported unless tests are excluded from analysis.

## Dependency Graph Signals

`dependency_cycle` and `dependency_hub` use a conservative source-file import
graph. The detector resolves only imports that point to another scanned source
file under the analysis root, such as relative JavaScript/TypeScript/Vue imports,
Rust `mod` declarations, Python relative imports, Ruby `require_relative`
calls, and quoted C/C++ includes that resolve to scanned source files.
Bash `source`/`.` and PowerShell dot-sourcing or `Import-Module` are not
modeled in this version.

- `dependency_cycle`: a resolved strongly connected component spans multiple
  source files. The Evidence reports cycle size, internal dependency edge count,
  and internal edge density.
- `dependency_hub`: a project with enough resolved graph data has a file with
  unusually high fan-in or fan-out. The Evidence reports direct fan-in/fan-out,
  transitive reach, dependency depth, and instability percentage so broad,
  deep, and mixed-responsibility hubs rank higher. Dependency depth is the
  longest path through the strongly connected component condensation graph.
  Files in the same cycle therefore share one component depth; cycle size and
  density remain evidence of `dependency_cycle` instead of being counted again
  as depth.

## Experimental Unity specialization

At a Unity project root, Reforge statically analyzes text serialization, meta GUIDs,
asmdef dependencies, Build Settings, and C# Unity component semantics without
starting the Editor. `Library/PackageCache` is identity-only external context and
never produces Issues. Missing PackageCache data or binary assets produce
`partially_observed` coverage instead of speculative broken-reference Evidence.

External packages, unresolved aliases, generated paths skipped by source filters,
and ambiguous language-specific module systems are ignored rather than guessed.

## Duplication and Test-Risk Signals

- `repeated_literal`: string or numeric literals occur at least
  `codebase.min-repeated-literal-occurrences` times after normalization and filtering.
- `repeated_error_pattern`: repeated catch/except/error handling patterns are
  found across supported languages.
- `data_clump`: repeated parameter groups occur at least
  `codebase.min-data-clump-occurrences` times.
- `test_duplication`: repeated setup, fixture, mock, fake, or before-hook
  patterns occur in tests.
- `happy_path_only_tests`: a test file has at least three test cases with
  assertion evidence but no negative, error, or boundary evidence.

Repeated-literal detection filters many report/fixture-like values, but
test-only and protocol literals remain important confounders to inspect.

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
Evidence groups generally provide stronger support than isolated matches.

## Exact adapter policy flow

`reforge.dataflow.adapter_flow_bypass` is an opt-in policy rule backed by an ordered exact
Rust value-transfer witness from a configured protected path to a configured
sink without visiting an allowed adapter path. The bounded source set is
function parameters declared in protected Rust paths. It remains separate from
`adapter_boundary_bypass`: the flow detector proves in-scope assignment,
argument, return, and project-local call edges; the drift detector remains a
naming and dependency heuristic.

The detector requires at least one `[[dataflow.policies]]` entry. It never
emits when a witness edge is unresolved, unsupported, or beyond
`max-function-hops`. Evidence carries numeric measurements and a typed Flow
witness. A conforming comparison
path is retained when observed but is not required before an exact bypass can
be reported. Workspace scans scope `crate::...` resolution to the nearest
owning Cargo package, while policy sink symbols remain source-level Rust paths.

## Documentation Signals

When the analysis root looks like a project, Reforge checks for a stable
documentation set.

- `missing_user_guide`: user-guide topics such as installation, quick start,
  CLI, configuration, output, and troubleshooting are missing.
- `missing_report_schema_docs`: JSON/YAML fields and compatibility
  expectations are undocumented.
- `missing_metrics_model_docs`: measurements, Evidence, Issues, or Coverage
  are undocumented.
- `missing_architecture_docs`: source collection, detector boundaries, data flow,
  or extension points are undocumented.
- `stale_cli_documentation`: docs mention CLI flags but omit current flags.
- `stale_schema_documentation`: report-schema docs omit current fields.

Expected docs include a docs index, user guide, configuration reference, report
schema, metrics model, detector reference, architecture guide, and contributing
guide.

## Interpreting Detector Output

Choose Issues using repository goals, measurements, cross-file spread, clear
locations, and Coverage. Treat heuristic Evidence as a prompt for inspection,
not an automatic refactor instruction.

`issues=0` means no unsuppressed Issues remain. It does not prove that the
scanned code is healthy or bug-free. Check Coverage and the suppression
summary when explaining an empty Issue list.

## Filtering and Suppression

Intentional Evidence can be suppressed with versioned `[[suppressions]]`
entries in `reforge.toml`. Each entry names a full rule, target-relative path,
reason, and optional line.

Suppressions remove matching Evidence before Issue aggregation. The suppression
summary preserves audit context so zero unsuppressed Issues is not mistaken for
zero observed signals.
