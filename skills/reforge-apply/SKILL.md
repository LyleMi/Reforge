---
name: reforge-apply
description: Apply an explicitly approved Reforge workflow plan within its frozen write set, preserving unrelated worktree changes and using sequential batches. Use only when the user explicitly invokes this skill for a run already in the `approved` phase.
---

# Reforge Apply

This skill is never implicit. Invoking it is not approval.

1. Run `reforge workflow status <run>` and `reforge workflow validate <run>`.
2. Continue only when `run.json.phase` is `approved` and `approval.json` matches the current `plan.json`. Never call `reforge workflow approve` from this skill.
3. Read repository instructions, the approved plan, approval write set, investigations, and relevant source/tests.
4. Apply batches sequentially. Edit only target-relative paths listed in `approval.json.write_set`. Preserve pre-existing user changes and avoid configuration, suppressions, generated output, commits, pushes, PRs, dependency installation, and network access unless separately authorized.
5. Stop if the implementation requires a new file, public boundary, behavior, or scope outside the approved plan. Return to planning and obtain a new explicit approval.
6. After edits, run `reforge workflow mark-applied <run>`. Treat rejection as a scope violation; do not hide or revert unrelated changes.
7. Hand the run to `$reforge-verify`. Do not claim that a successful edit or clean rescan proves behavior preservation.
