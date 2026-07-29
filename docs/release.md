# Release

The core release contains only the `reforge` binary and the `reforge-analyze`
skill. The installer and release archives must not add compatibility binaries
or optional workspace products.

Before publishing, run:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets --all-features
cargo build --release -p reforge
cargo run --locked -p reforge-calibrate -- corpus validate --manifest calibration/corpus.toml
cargo run --locked -p reforge-calibrate -- verify-promotion --corpus calibration/corpus.toml
```

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
