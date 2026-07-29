# Upgrading from 0.1 to 0.2

Reforge 0.2 intentionally does not read or migrate 0.1 reports or unversioned configuration.

- Replace `scan` and the separate `reforge-scan` / `reforge-flow` commands with `reforge analyze`. Codebase is the default; select `--analysis dataflow` explicitly or pass both `--analysis codebase --analysis dataflow`.
- Replace `analysis.lenses` with `analysis.enabled`; the removed key is rejected explicitly.
- Replace `catalog` with `rules`.
- Remove `--profile`, `--pack`, and Dataflow `mode`.
- Generate a new versioned configuration with `reforge init`; then reapply desired settings using the 0.2 configuration reference.
- Regenerate all older reports and baselines, including schema 26. Schema 27
  uses `ri7-*`/`re7-*`, typed entity subjects, content fingerprints, provenance,
  and per-Issue baseline states. No schema 26 compatibility reader exists.
- Rust producers must construct `Issue` with `Issue::new(IssueInput { .. })`
  and `Report` with `Report::new(ReportInput { .. })`. The old multi-argument
  constructors, `Issue::new_with_kind`, and `Report::new_with_provenance` were
  removed. `ReportInput.provenance` is mandatory so a producer cannot
  accidentally publish implicit analysis or rule provenance.
- Replace configuration version 1 with version 2 and add explicit
  `[rules].enable`, `disable`, and `enforce` arrays using complete rule IDs.
- Replace Dataflow `sink-symbols` with one single-language policy and
  `[[dataflow.policies.sinks]]` entries containing exact `path` and `symbol`.
- Restart workflow runs created with artifact v5. Artifact v6 verification
  reuses schema 27 baseline comparison and never treats `unknown` as resolved.
  Phase
  `Scanned` is now `Imported`, and plans use one `notes` list instead of
  investigation/conflict/unknown/alternative/batch fields.
- Unity is produced independently by `reforge-unity` as `reforge.unity`; it is
  not a core `reforge analyze --analysis` selection.
- Replace the removed `structure` analysis name, `[structure]` table, and `reforge.structure.*` namespace with `codebase`.

There is no runtime migration command. Keeping this boundary explicit prevents a converted report from claiming coverage that was never observed under the 0.2 rules.
