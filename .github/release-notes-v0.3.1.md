# Reforge v0.3.1

This release fixes the distributed skills' CLI version contract. The v0.3.0
archives contained a skill requiring CLI 0.2.0, causing agents following its
instructions to stop on a version mismatch. All distributed skills, the
investigator, and plugin manifests now match CLI 0.3.1.

## Highlights

- Version consistency tests compare distributed contracts with the Cargo
  package version. Release checks also reject mismatched tags and packaged skills.
- Codebase adds low-module-cohesion detection and improves first-run guidance.
- HTML reports and the bilingual documentation playground improve evidence review.
- Rust, report-app, and CI dependencies are updated.

Report schema 27 and workflow artifact schema 6 are unchanged. Dataflow still
reports partial coverage explicitly; empty Issues do not imply complete coverage.

## Upgrade

To replace both the binary and the stale v0.3.0 skill, use the release installer.

Unix:

```sh
curl -fsSL https://raw.githubusercontent.com/LyleMi/Reforge/v0.3.1/scripts/install.sh | sh -s -- --version v0.3.1
```

PowerShell:

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/LyleMi/Reforge/v0.3.1/scripts/install.ps1))) -Version v0.3.1
```

For the binary only:

```sh
cargo install reforge-cli --version 0.3.1 --locked
```

Expected binary output is `reforge 0.3.1`; the bundled skill requires CLI `0.3.1`.
The installers verify archive checksums and the binary version before installation.
Unity, workflow, and calibration remain optional repository components.
