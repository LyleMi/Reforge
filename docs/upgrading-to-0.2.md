# Upgrading from 0.1 to 0.2

Reforge 0.2 intentionally does not read or migrate 0.1 reports or unversioned configuration.

- Replace `scan` and the separate `reforge-scan` / `reforge-flow` commands with `reforge analyze`. Codebase is the default; select `--analysis dataflow` explicitly or pass both `--analysis codebase --analysis dataflow`.
- Replace `analysis.lenses` with `analysis.enabled`; the removed key is rejected explicitly.
- Replace `catalog` with `rules`.
- Remove `--profile`, `--pack`, and Dataflow `mode`.
- Generate a new versioned configuration with `reforge init`; then reapply desired settings using the 0.2 configuration reference.
- Regenerate all older reports and baselines, including schema 25. Schema 26 rejects `ri5-*`/`re5-*`; report identity, coverage, and Dataflow semantics are not migrated.
- Regenerate reports and baselines created with the earlier, unreleased schema
  26 shape: Issues now carry `analysis`, language coverage carries status and
  limitations, and rule coverage uses named observations.
- Restart workflow runs created with the earlier artifact v5 shape. Phase
  `Scanned` is now `Imported`, and plans use one `notes` list instead of
  investigation/conflict/unknown/alternative/batch fields.
- Unity is produced independently by `reforge-unity` as `reforge.unity`; it is
  not a core `reforge analyze --analysis` selection.
- Replace the removed `structure` analysis name, `[structure]` table, and `reforge.structure.*` namespace with `codebase`.

There is no runtime migration command. Keeping this boundary explicit prevents a converted report from claiming coverage that was never observed under the 0.2 rules.
