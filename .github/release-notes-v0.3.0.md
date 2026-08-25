# Reforge v0.3.0

Reforge is now available through crates.io in addition to the existing verified
binary installers. The installed command remains `reforge`.

## Highlights

- Install from crates.io with `cargo install reforge-cli --locked`.
- Publishable core crates are split into `reforge-schema`, `reforge-output`,
  `reforge-engine`, and the user-facing `reforge-cli` package.
- Offline HTML report assets are included in the published output crate.
- Unity, workflow, and calibration tools remain optional repository components
  and are not published to crates.io.
- Project documentation and onboarding now lead with the default Codebase
  analysis and its evidence-first review workflow.

## Install

Rust users:

```sh
cargo install reforge-cli --locked
reforge analyze .
```

The crates.io package installs the analyzer binary only. To install the binary
and bundled `reforge-analyze` Codex skill together, use the verified release
installer.

Unix:

```sh
curl -fsSL https://raw.githubusercontent.com/LyleMi/Reforge/v0.3.0/scripts/install.sh | sh -s -- --version v0.3.0
reforge analyze .
```

PowerShell:

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/LyleMi/Reforge/v0.3.0/scripts/install.ps1))) -Version v0.3.0
reforge analyze .
```

The release installers verify the downloaded archive against `SHA256SUMS`,
check the binary version, and install atomically.
