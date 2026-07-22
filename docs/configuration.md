# Configuration

Reforge can read scan defaults from `reforge.toml`. Command-line values take
precedence over configuration values.

## Discovery

When `--config` is not provided, Reforge looks for `reforge.toml` starting at
the scan root and walking upward through parent directories. If the scan root
is a file, discovery starts from that file's parent directory.

Use `--config <CONFIG>` to read a specific file:

```powershell
cargo run -- scan . --config D:\path\to\reforge.toml
```

## Commands

Create a default config in the current directory:

```powershell
cargo run -- init
```

Pass a directory to write `<PATH>\reforge.toml`, or pass a path ending in
`.toml` to write that exact file. Existing files are preserved unless
`--force` is supplied.

```powershell
cargo run -- init D:\path\to\project
cargo run -- init D:\path\to\project\reforge.toml --force
```

Validate a discovered or explicit config without scanning:

```powershell
cargo run -- config validate D:\path\to\project
cargo run -- config validate . --config D:\path\to\reforge.toml
```

Show effective scan defaults after applying discovered or explicit config:

```powershell
cargo run -- config show . --output human
cargo run -- config show . --output json
cargo run -- config show . --output yaml
```

`config validate` and `config show` parse `reforge.toml` but do not collect
source files, run detectors, or read git churn. The effective thresholds shown
by `config show` are used by detector findings.

## Precedence

Reforge applies presets and configuration as defaults. Threshold precedence is:
CLI per-threshold values, CLI `--preset`, `reforge.toml` per-threshold values,
`reforge.toml` `preset`, then the built-in `balanced` preset. A per-threshold
override is detected when its value differs from the built-in `balanced`
default, so a generated config can switch presets without deleting every
balanced threshold entry.

Boolean flags such as `--include-hidden`, `--include-generated`,
`--no-gitignore`, `--exclude-tests`, `--include-test-similarity`, and
`--include-test-structure` are CLI-only today. They are not read from
`reforge.toml`.

CI workflow flags such as `--baseline`, `--baseline-mode`, `--show`,
`--fail-on-findings`, `--output`, `--output-file`, `--reproducible`, `--progress`,
and `--color` are also CLI-only.

Finding filters `--only` and `--exclude-detector` are CLI-only. Long-lived
suppressions can be recorded in `reforge.toml`.

## Example

This example shows a tuned project configuration, not the built-in defaults.

```toml
preset = "strict"
max-file-lines = 600
max-dir-files = 35
max-function-lines = 60
max-function-complexity = 12
max-nesting-depth = 3
max-function-parameters = 4
max-type-lines = 200
max-type-members = 25
max-imports = 25
max-public-items = 20
max-functions-per-file = 40
max-functions-per-100-lines = 12
max-small-function-ratio = 70

min-similar-functions = 3
min-function-tokens = 70
function-similarity = 0.9
min-repeated-literal-occurrences = 5
min-data-clump-occurrences = 4

churn = "auto"
churn-window-days = 180
churn-max-commit-lines = 2000

ignore-paths = [
  "vendor",
  "generated/snapshots",
]

[[suppressions]]
kind = "large_file"
path = "src/generated.rs"
line = 1
reason = "generated fixture checked by snapshot tests"

[[suppressions]]
path = "src/legacy/generated.rs"
reason = "legacy migration tracked separately"

[data-flow]
mode = "policy"
max-hops = 4

[[data-flow.boundaries]]
name = "http-client"
protected-paths = ["src/domain", "src/application"]
adapter-paths = ["src/adapters/http"]
sink-symbols = ["crate::transport::send"]
exempt-paths = ["src/bin", "src/migrations"]
```

## Supported Keys

| Key | Default | Equivalent CLI option |
| --- | --- | --- |
| `preset` | `balanced` | `--preset` |
| `[unity].mode` | `auto` | `--unity` |
| `[unity].max-assembly-dependencies` | `8` | `--max-unity-assembly-dependencies` |
| `[unity].max-scene-objects` | `1000` | `--max-unity-scene-objects` |
| `[unity].max-prefab-objects` | `250` | `--max-unity-prefab-objects` |
| `[unity].max-serialized-fields` | `16` | `--max-unity-serialized-fields` |
| `[unity].max-lifecycle-methods` | `7` | `--max-unity-lifecycle-methods` |
| `max-file-lines` | `800` | `--max-file-lines` |
| `max-dir-files` | `40` | `--max-dir-files` |
| `min-similar-functions` | `3` | `--min-similar-functions` |
| `min-function-tokens` | `80` | `--min-function-tokens` |
| `function-similarity` | `0.85` | `--function-similarity` |
| `max-function-lines` | `80` | `--max-function-lines` |
| `max-function-complexity` | `15` | `--max-function-complexity` |
| `max-nesting-depth` | `4` | `--max-nesting-depth` |
| `max-function-parameters` | `5` | `--max-function-parameters` |
| `max-type-lines` | `250` | `--max-type-lines` |
| `max-type-members` | `30` | `--max-type-members` |
| `max-imports` | `35` | `--max-imports` |
| `max-public-items` | `30` | `--max-public-items` |
| `max-functions-per-file` | `40` | `--max-functions-per-file` |
| `max-functions-per-100-lines` | `12` | `--max-functions-per-100-lines` |
| `max-small-function-ratio` | `70` | `--max-small-function-ratio` |
| `min-repeated-literal-occurrences` | `12` | `--min-repeated-literal-occurrences` |
| `min-data-clump-occurrences` | `4` | `--min-data-clump-occurrences` |
| `churn` | `auto` | `--churn` |
| `churn-window-days` | `180` | `--churn-window-days` |
| `churn-max-commit-lines` | `2000` | `--churn-max-commit-lines` |
| `ignore-paths` | `[]` | `--ignore-path` |
| `suppressions` | `[]` | none |
| `[data-flow].mode` | `off` | none; config-owned |
| `[data-flow].max-hops` | `4` | none; config-owned |
| `[[data-flow.boundaries]]` | `[]` | none; config-owned |

`preset` accepts `strict`, `balanced`, or `relaxed`. `churn` accepts `auto`,
`on`, or `off`.

## Exact Rust Data Flow

Data-flow analysis is opt-in and configuration-owned. `off` allocates no flow
graph. `observe` builds exact Rust facts and coverage receipts but emits no
finding. `policy` evaluates configured boundaries and can emit
`adapter_flow_bypass`.

Each boundary requires a unique non-empty `name`, at least one
`protected-paths`, `adapter-paths`, and `sink-symbols` entry, and may declare
`exempt-paths`. Paths are root-relative glob patterns; a plain directory also
matches its descendants. Sink symbols must be complete `crate::...` Rust
free-function symbols. `max-hops` counts interprocedural call edges and must be
greater than zero.

The resolver covers project-local free functions, positional arguments,
lexical aliases, returns, `crate`, `self`, `super`, explicit module paths, and
unambiguous imports/re-exports. Unsupported fields, methods, trait dispatch,
macros, escaping closures, external crates, async causality, and library calls
degrade coverage instead of becoming speculative witness edges.

`config validate` rejects unknown keys, invalid modes/globs, duplicate or empty
boundary names, zero hops, incomplete boundaries, and non-qualified sink
symbols. Finding filters, suppressions, and baselines apply to emitted findings
without hiding raw `flow_analysis` coverage.

## Choosing Parameters

Reforge thresholds are review-policy defaults, not universal definitions of
maintainable code. ISO/IEC 25010 informs the maintainability constructs used to
classify findings, but it does not prescribe limits such as 800 lines per file
or complexity 15. The defaults combine conservative engineering heuristics
with cross-project calibration samples. Treat them as a starting point and
validate them against the kinds of repositories your team actually maintains.

The presets change detector sensitivity as a group:

| Parameter | Strict | Balanced | Relaxed |
| --- | ---: | ---: | ---: |
| Maximum file lines | 600 | 800 | 1200 |
| Maximum direct files per directory | 30 | 40 | 60 |
| Minimum similar functions per group | 2 | 3 | 4 |
| Minimum function tokens for similarity | 60 | 80 | 120 |
| Function similarity | 0.88 | 0.85 | 0.90 |
| Maximum function lines | 60 | 80 | 120 |
| Maximum function complexity | 12 | 15 | 20 |
| Maximum nesting depth | 3 | 4 | 5 |
| Maximum function parameters | 4 | 5 | 6 |
| Maximum type lines | 200 | 250 | 400 |
| Maximum type members | 25 | 30 | 45 |
| Maximum imports | 25 | 35 | 50 |
| Maximum public items | 20 | 30 | 45 |
| Maximum functions per file | 35 | 40 | 60 |
| Maximum functions per 100 lines | 10 | 12 | 18 |
| Maximum small-function ratio | 65 | 70 | 80 |
| Minimum repeated-literal occurrences | 8 | 12 | 20 |
| Minimum data-clump occurrences | 3 | 4 | 6 |
| Maximum Unity assembly dependencies | 5 | 8 | 12 |
| Maximum Unity scene objects | 500 | 1000 | 2000 |
| Maximum Unity prefab objects | 100 | 250 | 500 |
| Maximum Unity serialized fields | 10 | 16 | 24 |
| Maximum Unity lifecycle methods | 5 | 7 | 10 |

`strict` is intended for mature codebases that accept more review prompts in
exchange for earlier signals. `balanced` favors useful coverage without making
ordinary variation immediately actionable. `relaxed` is appropriate for an
initial rollout, framework-heavy code, or repositories where generated-style
or orchestration code is common. Presets do not change finding semantics or
prove that a repository meets a quality standard.

Similarity sensitivity is determined by three parameters together. A lower
token minimum admits shorter functions, a lower group minimum admits smaller
duplication clusters, and a lower similarity value admits more structural
variation. The strict preset uses a slightly higher similarity value than the
balanced preset to offset the extra noise introduced by admitting shorter
functions and two-member groups. The relaxed preset raises all three admission
requirements. Tune the three controls together rather than interpreting the
similarity percentage in isolation.

Structural limits such as file size, function size, complexity, nesting, and
type size are absolute review budgets. The balanced file and function limits
are broadly consistent with upper-decile or upper-tail observations in the
maintainer samples, but other limits have less direct empirical support. See
[Calibration Samples](calibration-samples.md) for the available evidence and
its limitations. Lower a limit when maintainers repeatedly agree that findings
below the current boundary are actionable; raise it when the same project
pattern is repeatedly reviewed and rejected.

Repetition thresholds trade recall for evidence strength. Low values find
smaller repeated-literal and data-clump patterns but are more sensitive to
fixtures, protocol constants, and framework conventions. High values require a
broader repeated pattern before recommending consolidation. Review occurrences
and related locations before changing these thresholds globally.

Unity thresholds are initial operational heuristics, not values derived from a
public Unity benchmark. They represent review points for assembly fan-out,
serialized asset size, component state breadth, and lifecycle responsibility.
Start with `--unity auto`; use `on` when CI must reject a path that is not a
recognizable Unity root, and `off` when Unity assets are intentionally outside
the audit. Calibrate the five Unity thresholds against representative scenes,
prefabs, assemblies, and behaviours before using them as blocking policy.

The churn defaults use a 180-day window to emphasize recent maintenance
pressure while retaining enough history for lower-activity repositories.
Commits above 2000 added-plus-deleted lines are skipped to reduce domination by
bulk formatting, vendoring, or generated updates. These are operational
defaults rather than statistically universal cutoffs. Shorten the window for
fast-moving products, lengthen it for stable libraries, and adjust the commit
limit when legitimate repository changes are routinely larger or smaller.

For a new policy, begin with `balanced`, keep `--churn auto` for exploratory
audits, and use `--churn off --reproducible` when comparing byte-stable
source-only reports.
Review findings with maintainers across several representative repositories,
then validate proposed changes on a holdout repository. For CI, capture a
schema 24 baseline and use `--baseline-mode new --fail-on-findings` so
unchanged legacy evidence remains visible without blocking every change.

## Ignored Paths

`ignore-paths` entries are relative to the scan root. Both `\` and `/` are
normalized to `/`, and leading or trailing slashes are ignored. An ignored
entry matches the path itself and any descendant path.

Example:

```toml
ignore-paths = ["vendor", "src/generated"]
```

This skips `vendor`, `vendor/foo.rs`, `src/generated`, and
`src/generated/schema.ts`.

The built-in generated/dependency exclusions still apply unless
`--include-generated` is passed.

Reforge also applies `.gitignore`, `.git/info/exclude`, and global git ignore
rules by default. Use `--no-gitignore` when you intentionally want to scan
paths ignored by git. `--include-generated` only disables Reforge's built-in
generated/dependency directory list; it does not override git ignore rules.

## Suppressions

Use suppressions for intentional findings that should be absent from reports
and CI gates. Suppressions remove matching entries from `findings`.
Suppression summary context should stay visible in reviews so a report with
zero findings is read as zero unsuppressed findings, not as proof that no
maintainability signals were measured.
Config suppressions use TOML tables:

```toml
[[suppressions]]
kind = "large_file"
path = "src/generated.rs"
line = 1
reason = "generated fixture"
```

`path` is required and is matched relative to the scan root. Both `\` and `/`
separators are accepted. `kind` is optional; when omitted, every finding kind
on that path can match. `line` is optional; when omitted, the whole path can
match. `reason` is required and must be non-empty.

Inline comments can suppress findings near the source:

- `reforge:ignore [kind[,kind...]] reason` suppresses same-line findings.
- `reforge:ignore-next-line [kind[,kind...]] reason` suppresses next-line
  findings.
- `reforge:ignore-file [kind[,kind...]] reason` suppresses matching findings
  anywhere in that file.

When no kind list is provided, an inline suppression matches every finding kind
in its scope. Unknown kinds in CLI filters, config suppressions, or kind-like
inline suppression tokens fail the scan with a clear error.
