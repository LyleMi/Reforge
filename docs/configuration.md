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

## Precedence

Reforge applies configuration as defaults. A threshold from `reforge.toml` is
used only when the CLI value is still the built-in default. Explicit CLI values
win.

Boolean flags such as `--include-hidden`, `--include-generated`,
`--include-test-similarity`, and `--include-test-structure` are CLI-only today.
They are not read from `reforge.toml`.

## Example

```toml
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

min-similar-functions = 3
min-function-tokens = 70
function-similarity = 0.9
min-repeated-literal-occurrences = 5
min-data-clump-occurrences = 4

churn = "auto"
hotspot-model = "hybrid"
churn-window-days = 180
churn-max-commit-lines = 2000

ignore-paths = [
  "vendor",
  "generated/snapshots",
]
```

## Supported Keys

| Key | Default | Equivalent CLI option |
| --- | --- | --- |
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
| `min-repeated-literal-occurrences` | `4` | `--min-repeated-literal-occurrences` |
| `min-data-clump-occurrences` | `3` | `--min-data-clump-occurrences` |
| `churn` | `auto` | `--churn` |
| `hotspot-model` | `hybrid` | `--hotspot-model` |
| `churn-window-days` | `180` | `--churn-window-days` |
| `churn-max-commit-lines` | `2000` | `--churn-max-commit-lines` |
| `ignore-paths` | `[]` | none |

`churn` accepts `auto`, `on`, or `off`. `hotspot-model` accepts `static`,
`churn`, or `hybrid`.

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
