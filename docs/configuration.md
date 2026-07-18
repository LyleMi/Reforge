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
by `config show` are used by both threshold findings and static hotspot risk.

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
`--fail-on`, `--output`, `--output-file`, `--progress`, and `--color` are also
CLI-only.

Finding filters such as `--only`, `--exclude-detector`, `--min-priority`, and
`--severity` are CLI-only. Long-lived suppressions can be recorded in
`reforge.toml`.

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
hotspot-model = "hybrid"
scoring-policy = "policies/accepted-policy.json"
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
| `hotspot-model` | `hybrid` | `--hotspot-model` |
| `scoring-policy` | none | `--scoring-policy` |
| `churn-window-days` | `180` | `--churn-window-days` |
| `churn-max-commit-lines` | `2000` | `--churn-max-commit-lines` |
| `ignore-paths` | `[]` | `--ignore-path` |
| `suppressions` | `[]` | none |

`preset` accepts `strict`, `balanced`, or `relaxed`. `churn` accepts `auto`,
`on`, or `off`. `hotspot-model` accepts `static`, `churn`, or `hybrid`.

Only accepted policy v1 files can be loaded. A CLI path resolves from the
current working directory and takes precedence over configuration. A relative
`reforge.toml` path resolves from the configuration file directory. Unknown
fields or detector kinds, invalid reliability ranges, weights that do not sum
to one, version mismatch, non-accepted status, or fingerprint mismatch are
fatal configuration errors.

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
and CI gates. Suppressions remove matching entries from `findings`; they do not
remove hotspot watchlist entries, because hotspots are ranked from raw metrics.
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
