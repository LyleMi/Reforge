# Release

This checklist is for maintainers preparing a Reforge release.

## Pre-Release Checks

Confirm the crate metadata in `Cargo.toml`:

- `version`
- `rust-version`
- `description`
- `license`
- `readme`
- `repository`
- `homepage`
- `documentation`
- `keywords`
- `categories`

Run validation:

```powershell
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features
cargo run -- scan . --output json --progress never --churn off
```

Review the generated JSON for unexpected schema or detector changes.

## Schema Review

If serialized report shape changed:

- Increment `SCAN_REPORT_SCHEMA_VERSION` in `src/model/mod.rs`.
- Update `docs/report-schema.md`.
- Update README references to the schema version.
- Add or update output tests.

If only detector behavior or scoring changed without report shape changes, do
not increment the schema version solely for ranking changes unless consumers
need a compatibility boundary.

## Documentation Review

Before tagging, check:

- `README.md` quick start still works.
- `docs/user-guide.md` includes all current CLI flags.
- `docs/configuration.md` matches config keys in `scan/mod.rs`.
- `docs/report-schema.md` matches the serialized `ScanReport` model.
- `docs/detectors.md` lists current `FindingKind` values and detector
  families.

## Packaging

Build a release binary:

```powershell
cargo build --release
```

Install locally from the release candidate:

```powershell
cargo install --path .
reforge scan . --progress never
```

## Release Notes

Release notes should include:

- New detector or CLI capabilities.
- Changed thresholds, scoring, output, or schema.
- Bug fixes that affect scan results.
- Any compatibility notes for JSON/YAML consumers.
