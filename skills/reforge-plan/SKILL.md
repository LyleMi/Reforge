---
name: reforge-plan
description: Investigate user-selected Reforge schema 21 issues and produce validated, resumable workflow investigation and plan artifacts without editing source. Use when Codex is asked to analyze Reforge issue IDs, estimate refactor scope and tests, build conflict-aware batches, or prepare work for explicit approval with `reforge workflow`.
---

# Reforge Plan

Keep source, project configuration, suppressions, and shared workflow state read-only while investigating.

1. Start or load a workflow:

```bash
reforge workflow start <target> --progress never
reforge workflow select <run> --issue ri3-... --goal "desired outcome"
reforge workflow status <run>
```

2. Read `run.json`, `scan.json`, `selection.json`, repository instructions, selected issues, member findings, coverage receipts, `agent_evidence`, source, and relevant tests. Never select by an invented score.
3. Route each issue to one reference: [structure](references/structure.md), [duplication and drift](references/duplication-drift.md), [dependency](references/dependency.md), [documentation](references/documentation.md), or [Unity](references/unity.md).
4. For independent issues, ask Codex to use at most four `reforge-investigator` agents. Give each agent exactly one selected issue and require returned JSON only. The coordinator alone writes `investigations/<issue-id>.json` and shared artifacts.
5. Make every investigation artifact schema v2 and include exactly: `artifact_schema_version`, `issue_id`, `finding_ids`, `report_fingerprint`, `status`, `facts`, `analysis`, `unknowns`, `rejected_alternatives`, `inspected_files`, `read_set`, `write_set`, `coverage_limitations`, and `checks`. Use target-relative paths. Separate repository facts from interpretation.
6. Run `reforge workflow advance <run>`. A complete set moves `selected` to `investigated`; incomplete evidence becomes `needs_input` or `failed`.
7. Write `plan.json` with exactly: `artifact_schema_version`, `report_fingerprint`, `goal`, `outcome`, `selected_issue_ids`, `batches`, `write_set`, `behavior_assumptions`, `checks`, `unresolved_risks`, and `conflicts`. Every write path must have been proposed by an investigation. Each check has `kind`, `program`, `args`, `required`, and `expected_observation`.
8. Keep conflicting issues out of the same batch. Include the CLI-computed conflict edges implied by shared evidence/write files, dependency boundaries, tests, or Unity surfaces.
9. Run `reforge workflow advance <run>` and then `reforge workflow validate <run>`. Stop in `planned`. Do not run `approve`, edit source, or imply that planning is authorization.

Do not change thresholds or add suppressions to obtain a clean rescan. Report partial coverage, unresolved dependencies, missing tests, user worktree overlap, and behavior assumptions as visible limitations.
