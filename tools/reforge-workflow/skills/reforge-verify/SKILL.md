---
name: reforge-verify
description: Optional guard workflow that verifies an Applied artifact v5 workflow with direct checks and fresh `reforge analyze` schema 26 reports.
---

# Reforge Verify

Generated contract: CLI `0.2.0`, report schema `26`, artifact schema `5`.

1. Check `reforge-workflow --version`, `status`, and `validate`; require `Applied`.
2. Run every required command directly with `reforge-workflow check --kind
   <test|build|lint|custom> -- <program> <args...>`. Never use a shell wrapper.
3. Generate a fresh report with `reforge analyze`. Workflow does not invoke analyzers.
4. Finish with `reforge-workflow verify --report <file>...`.
5. Report the recorded `pass`, `failed`, or `needs_input` outcome. Pass requires every selected
   Issue to disappear, all checks (including a test) to pass, no coverage downgrade, no new Issue,
   and consistent workspace/write-set state. Missing producers or unobservable coverage needs
   input; never relabel it as pass.
