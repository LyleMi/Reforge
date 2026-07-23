# Release

The core release contains only the `reforge` binary and the `reforge-analyze`
skill. The installer and release archives must not add compatibility binaries
or optional workspace products.

Before publishing, run:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
cargo build --release -p reforge
```

Also run report-app unit, browser, and build checks; Codebase, Dataflow, and
combined reproducible self-analysis; and smoke tests for every output format
plus the two explicit debug sidecars.

Use a Conventional Commit message and document user-visible changes, validation
commands, linked issues, and sample output changes in the pull request.
