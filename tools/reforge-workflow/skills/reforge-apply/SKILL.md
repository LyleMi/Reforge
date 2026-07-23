---
name: reforge-apply
description: Optional guard workflow that applies only an explicitly approved Reforge artifact v5 plan within its frozen write set.
---

# Reforge Apply

Generated contract: CLI `0.2.0`, report schema `26`, artifact schema `5`.

1. Check `reforge-workflow --version`, `status`, and `validate`; stop on mismatch.
2. Continue only in `Approved`. Never approve from this skill.
3. Read `plan.json` and `approval.json`. Change only approved write-set paths and implement only
   selected Issue work. Preserve unrelated user changes.
4. Do not change configuration, suppressions, thresholds, checks, or the plan hash.
5. Run `reforge-workflow mark-applied`. Treat any write-set rejection as a scope violation and
   stop for user direction.
