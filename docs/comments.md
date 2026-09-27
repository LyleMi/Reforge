# Pick and clean comments

Use `reforge comments pick` to review comments, and `reforge comments clean` to
preview or apply explicitly selected removals. Both commands run locally.
They support Rust, JavaScript/JSX and TypeScript/TSX, including `.mjs`, `.cjs`,
`.mts` and `.cts`. Vue, other languages and files with syntax/encoding errors
are not editable and appear in the skipped-file receipts when discovered.

## Pick a review set

```sh
reforge comments pick .
reforge comments pick . --candidates
reforge comments pick src --contains 'legacy' --kind line
reforge comments pick . --output json --output-file comments.json
```

The inventory contains the original comment text, path, line range, enclosing
or following symbol when available, nearby source, protection reasons and
review hints. Consecutive standalone ordinary line comments are one group:
selecting a matching line selects the whole group. This preserves continuation
lines of directives, licenses and rationale. Inspect the group before cleaning.
Inline comments and block comments are separate items.

Review hints cover empty/separator comments, repeated text within one file,
known template placeholders and possible commented-out code. They are
heuristics, not proof of uselessness or obsolescence. Reforge does not infer
whether an arbitrary explanation is stale and does not use a model to rewrite
comments.

Selection flags:

| Flag | Meaning |
| --- | --- |
| `--contains TEXT` | Case-sensitive literal substring of the original comment |
| `--text TEXT` | Exact trimmed body without comment delimiters; multiline bodies retain line breaks |
| `--id ID` | Exact ID from the inventory |
| `--kind line\|block\|documentation` | Limit comment kind |
| `--candidates` | Limit to comments with review hints |

Repeated values within one selector are ORed. Different selectors are ANDed.
Empty text selectors are rejected. IDs include the file content hash: any change
to that file makes its old IDs stale, which produces an error instead of silently
selecting another comment. JSON inventory/plan output is deterministic for an
unchanged root and configuration. Byte ranges refer to decoded UTF-8 without
its BOM, not byte offsets in a UTF-16 file.

The commands discover `reforge.toml` and honor its `[scope]` settings. The scope
flags `--config`, `--include-hidden`, `--include-generated`, `--no-gitignore`,
`--exclude-tests` and repeatable `--ignore-path` are also available. Generated
folders such as `target`, `node_modules`, `dist` and `build` are excluded by
default. `--ignore-path` matches relative paths/directory prefixes, not globs.
Excluded files are outside the inventory; discovered but unsupported or invalid
files are listed as skipped. Other analyzer configuration is validated but does
not enable or disable comment inventory or cleanup.

## Preview and apply

```sh
# Preview: source files stay unchanged.
reforge comments clean . --text 'your code here'

# Apply the selected ordinary comment removals in one command.
reforge comments clean . --text 'your code here' --apply

# Or select an exact comment group from pick.
reforge comments clean . --id comment-<full-id>
```

Clean requires `--all`, `--id`, `--text` or `--contains`. `--candidates` alone does not
authorize deletion. Documentation comments, license notices, recognized tool
instructions (including `reforge:`, `@ts-ignore`, ESLint and source-map markers),
TODO/FIXME debt, generated notices and recognized safety/rationale statements
are protected in text/ID selection mode, even if the selector matches them.
Protection is conservative and marker-based; a plain comment with important
meaning but no recognized marker still requires human judgment.

The default output is a unified diff on stdout. A summary and skipped-file
receipts go to stderr. A zero-match or fully protected selection produces no
changes. Applying a selection skips protected comments and reports their count.
To see their specific reasons, use `pick` with the same selectors.

To remove **every comment**, including protected documentation, licenses,
TODO/FIXME and tool directives:

```sh
# Preview every removal in the configured scope.
reforge comments clean . --all

# One-command application.
reforge comments clean . --all --apply

# Or save and replay the full-removal plan.
reforge comments clean . --all --output json --output-file all-comments.json
reforge comments clean . --plan all-comments.json --apply
```

`--all` is mutually exclusive with text/ID/kind/candidate selectors and `--plan`.
It still honors scope exclusions and supported languages. Unsupported or invalid
files are reported as skipped; strings containing comment delimiters remain
unchanged. Hash checks, syntax checks and write recovery still apply. Deleting
documentation or tool directives can change generated docs, lint/type-check
results or other tooling behavior even when non-comment syntax is unchanged.
The summary explicitly identifies all-comments mode, including when replaying a
saved plan. `protected_comments` counts comments retained by protection, so it
is zero in this mode.

Save a plan when review and application happen separately:

```sh
reforge comments clean . --text 'your code here' \
  --output json --output-file cleanup.json
reforge comments clean . --plan cleanup.json
reforge comments clean . --plan cleanup.json --apply
```

Pass the original workspace path when replaying a plan from a different working
directory. A plan cannot be combined with new selection or scope flags. Plans
use version `1` for protected text/ID selection and version `2` with
`removal_mode: "all"` for full removal. Existing version 1 plans retain their
original protection behavior. These are not schema 27 reports or workflow artifacts. The plan records selected IDs, file hashes, original edit text and
replacement ranges. Application validates the workspace root, hashes, selection,
the recorded removal mode and recomputed edits; arbitrary modified replacements are rejected.
To narrow a plan, generate another plan with the desired IDs rather than editing
its ranges or replacement text.

Output files must be new files with `.json`, `.diff`, `.patch` or `.txt`
extensions. Existing output files are never overwritten. Choose another name
or remove an obsolete artifact yourself.

## Write behavior and limits

Whole standalone comment lines are removed. Inline comments are replaced with
whitespace that separates neighboring tokens and preserves embedded line
terminators. UTF-8, UTF-8 BOM, UTF-16 LE/BE BOM, retained CRLF/LF and file
permissions are preserved. The result must parse and retain the same
non-comment syntax tree, including punctuation. Ambiguous JavaScript automatic
semicolon insertion changes are rejected. This syntax check does not establish
that deleting an explanation is useful or preserve every external tool's
interpretation of unknown comment directives.

Every edited file is validated and staged before writing. Files changed after
selection, symlink paths, hard-linked files and read-only files are rejected.
Ordinary commit failures attempt to restore earlier writes and report any
rollback failure. Multi-file application is not crash-atomic; avoid concurrent
writers while applying. Use source control to revert an applied cleanup.

Diffs display decoded text. Use `--apply` or plan replay to preserve BOM and
UTF-16 encoding; external patch tools are not an encoding-preserving substitute.

## Codebase report integration

Enable the advisory rule to include comment review hints in the regular
Codebase report:

```sh
reforge analyze . --analysis codebase \
  --set "rules.enable=['reforge.codebase.comment_hygiene']"
```

Or add `reforge.codebase.comment_hygiene` to `[rules].enable` in `reforge.toml`.
It is a preview, default-off rule for Rust, JavaScript and TypeScript/TSX. Evidence
includes the comment location and hint; protected comments do not produce hints.
It belongs to the documentation-integrity family and uses the shared parsed
workspace sources. Dataflow-only execution does not run it. The rule cannot be
enforced as policy while it is preview. Analysis never modifies source files.
