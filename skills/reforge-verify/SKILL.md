---
name: reforge-verify
description: Verify an applied Reforge workflow by running declared formatter, build, test, and custom commands without a shell, rescanning with the original effective configuration, and classifying removed, remaining, new, or unobservable evidence. Use after `reforge workflow mark-applied` or to resume incomplete verification.
---

# Reforge Verify

1. Run `reforge workflow status <run>` and `reforge workflow validate <run>`. Require an applied workflow with unchanged approval and write-set boundaries.
2. Read the required checks from `plan.json`. Execute each exact command through:

```bash
reforge workflow check <run> --kind format -- <program> <args...>
reforge workflow check <run> --kind build -- <program> <args...>
reforge workflow check <run> --kind test -- <program> <args...>
```

Use `custom` for declared repository-specific checks. Commands execute directly, never through a shell. Do not place secrets in command arguments or output.
3. If a check fails, times out, or is missing, preserve the recorded result and diagnose within the approved scope. Never relabel a failure as verified.
4. Run `reforge workflow rescan <run>` once. Read `rescan.json` as four separate outcomes: selected evidence removed, selected evidence still present, new evidence, and unobservable evidence. A rescan is supporting evidence, not a test substitute.
5. Run `reforge workflow finish <run>`. `verified` requires unchanged scope, all required checks successful, a required test check, and completed observable rescan. Missing tests or degraded observation becomes `needs_input`; failed checks become `failed`.
6. Report checks, evidence comparison, suppressions, coverage limitations, and remaining unknowns. Do not commit, push, open a PR, tune thresholds, or add suppressions.
