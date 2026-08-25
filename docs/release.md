# Release

The core release contains only the `reforge` binary and the `reforge-analyze`
skill. The installer and release archives must not add compatibility binaries
or optional workspace products.

Release archives are `reforge-linux-x86_64.tar.gz`,
`reforge-macos-x86_64.tar.gz`, `reforge-macos-aarch64.tar.gz`, and
`reforge-windows-x86_64.zip`. Each contains the native binary, README, LICENSE,
and `reforge-analyze` skill. The release also publishes `SHA256SUMS`; the remote
installers require it and verify the extracted binary's version before an
atomic install.

Before publishing, run:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets --all-features
cargo build --release -p reforge
cargo run --locked -p reforge-calibrate -- corpus validate --manifest calibration/corpus.toml
cargo run --locked -p reforge-calibrate -- verify-promotion --corpus calibration/corpus.toml
```

The crates.io distribution uses package name `reforge-cli` while continuing to
install the `reforge` binary. Publish the core packages in dependency order:

```sh
cargo publish -p reforge-schema
cargo publish -p reforge-output
cargo publish -p reforge-engine
cargo publish -p reforge-cli
```

Before the first publication of a version, use `cargo package --list` to inspect
dependent package contents because Cargo requires their matching registry
dependencies to exist even with `--no-verify`. After each dependency is
available on crates.io, run `cargo package` without `--no-verify` before
publishing the next package. The Unity, workflow, and calibration packages are
not published.

Also run report-app unit, browser, and build checks; Codebase, Dataflow, and
combined reproducible self-analysis; deterministic and policy-gate checks;
frozen-corpus calibration and five-language differential snapshots; and smoke
tests for every output format plus the two explicit debug sidecars. Report-app
source changes must regenerate and commit both embedded assets with a clean
asset diff.

The reusable `calibration.yml` workflow builds the current release binary,
checks out all 15 typed matrix entries at their fixed commits, runs isolated
Codebase, isolated Dataflow, and combined analysis twice, and verifies
byte-level determinism, isolation/union, coverage, workspace/revision identity,
and sidecars. It uploads complete reports plus SHA-256 audit manifests. It
does not call an LLM or create labels. The release workflow must complete that
workflow before packaging and then run `verify-promotion` over all audit
manifests. A stable or default-enabled rule without matching rule/language
review evidence bound to the current corpus and report-audit digests fails
closed. A preview-only registry reports zero promotion candidates and passes.

Use a Conventional Commit message and document user-visible changes, validation
commands, linked issues, and sample output changes in the pull request.
