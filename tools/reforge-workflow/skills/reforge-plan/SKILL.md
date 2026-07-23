---
name: reforge-plan
description: Optional guard workflow that builds an artifact v5 plan from selected schema 26 Reforge Issues without editing source.
---

# Reforge Plan

Generated contract: CLI `0.2.0`, report schema `26`, artifact schema `5`.

1. Check `reforge-workflow --version`; stop on mismatch.
2. Generate a combined `reforge analyze` report, then explicitly start the guard: `reforge-workflow start --report report.json --goal "..."`.
3. Inspect selected Issue evidence, relevant code, coverage limitations, intended paths, and
   checks. Record important decisions or uncertainty in `notes`. Do not edit source.
4. Produce one complete `plan.json` containing artifact schema 5, the exact goal, selected Issue
   IDs, notes, changes, a target-relative write set, and required checks. At least
   one required check must be `test`.
5. Import it with `reforge-workflow plan --artifact plan.json`, then run `status` and `validate`.
   Stop in `Planned`; planning is not approval.
