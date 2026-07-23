# Optional agent guard

`reforge-workflow` is optional; normal analysis does not require it. It consumes reports and never invokes another Reforge tool. All input reports must
be schema 26 and share workspace identity and source revision. Conflicting content for one Issue ID
is rejected.

```bash
reforge analyze . --output json --output-file report.json --reproducible
reforge-workflow start --report report.json --goal "split the boundary"
reforge-workflow plan --artifact plan.json
reforge-workflow approve
reforge-workflow mark-applied
reforge-workflow check --kind lint -- cargo clippy --workspace
reforge-workflow check --kind test -- cargo test --workspace
reforge-workflow verify --report report-after.json
```

Commands accept `--run <directory>` and otherwise use `.reforge-workflow`. `status` and `validate`
are read-only.

Artifact v5 has five phases: `Imported -> Planned -> Approved -> Applied -> Verified`. The imported
plan contains selection, notes, changes, a target-relative write set, and required checks. Artifact
v5 files using the earlier migration shape must be regenerated. Approval freezes the plan hash,
write set, and workspace snapshot.
`mark-applied` rejects any changed path outside that set. Checks execute a program directly with a
timeout and no shell.

Verification passes only when selected Issues disappear, every required check succeeds, at least
one test check succeeds, relevant coverage does not degrade, no new Issue appears, and workspace
scope remains consistent. Missing reports/producers or unobservable coverage produces
`needs_input`; remaining/new Issues, failed checks, or coverage downgrade produces `failed`.

The run contains only `run.json`, `reports/`, `plan.json`, `approval.json`, `application.json`,
`checks.json`, and `verification.json`. Older artifact versions are rejected.
