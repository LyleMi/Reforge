# User guide

## Install

The verified release installer chooses the supported asset for the current OS and CPU, validates it against the release `SHA256SUMS`, checks `reforge --version`, and atomically installs the binary. It also installs the `reforge-analyze` Codex skill unless disabled.

Unix:

```sh
curl -fsSL https://raw.githubusercontent.com/LyleMi/Reforge/main/scripts/install.sh | sh
# Pin a release or choose a destination:
curl -fsSL https://raw.githubusercontent.com/LyleMi/Reforge/main/scripts/install.sh | \
  sh -s -- --version v0.2.0 --bin-dir "$HOME/.local/bin"
```

The Unix default is `${REFORGE_INSTALL_DIR:-$HOME/.local/bin}`. Supported assets are Linux x86_64 and macOS x86_64/aarch64.

PowerShell:

```powershell
$installer = Join-Path $env:TEMP "install-reforge.ps1"
irm https://raw.githubusercontent.com/LyleMi/Reforge/main/scripts/install.ps1 -OutFile $installer
& $installer
# Pin a release or choose a destination:
& $installer -Version v0.2.0 -BinDir C:\Tools\Reforge
```

Windows x86_64 defaults to `%LOCALAPPDATA%\Reforge\bin`. Use `--skip-skill` or `-SkipSkill` to install only the binary. Neither installer edits PATH; when necessary it prints the exact command to add the selected directory. Re-running an installer safely replaces the same version or upgrades it.

From a source checkout, the existing `scripts/install-reforge.sh`, `scripts/install-reforge.ps1`, and `.bat` wrapper remain available for `cargo install --path` development workflows.

## Analyze

Run the default Codebase analysis:

```sh
reforge analyze . --reproducible
```

Without a configuration file, the CLI enables a small starter set of preview
advisories for large files, long functions, dependency cycles, and similar
functions. They produce review prompts but cannot fail a policy gate. Run
`reforge init` to write the same starter selection to `reforge.toml`, then tune
or disable it for the repository.

Dataflow is explicit. Run it alone or combine both core analyses over one workspace index:

```sh
reforge analyze . --analysis dataflow --output json --reproducible
reforge analyze . --analysis codebase --analysis dataflow --reproducible
```

Use `--output` and `--output-file` for human, HTML, JSON, YAML, or SARIF reports. Raw Codebase metrics and the complete Flow IR are opt-in debug sidecars:

```sh
reforge analyze . --analysis codebase --metrics-output metrics.json
reforge analyze . --analysis dataflow --flow-ir-output flow-ir.json
```

## Read a report

Treat `issues` as the only decision units. Each Issue owns one typed subject and one or more Evidence records. Evidence identifies the rule and may include measurements, locations, and an ordered Dataflow witness.

Read Coverage before interpreting absence. Check the selected analysis status, every language receipt, capability limitation, rule execution, and suppression count. An empty Issue list is an observed zero only where Coverage is observable. Dataflow never represents a partial or unresolved path as exact.

Reforge intentionally emits no health score, severity, priority, or defect probability.

## Baselines and CI gates

A baseline must use a compatible report format with the same producer, identity scheme, and workspace identity. Producer versions and unrelated analysis sets may differ. Coverage, scope, configuration, policy, or rule-semantic changes produce an `unknown` baseline state instead of claiming that an Issue is new or resolved.

After reviewing and storing a baseline report, gate new, updated, or unknown policy Issues with:

```sh
reforge analyze . --output json --output-file current.json \
  --baseline reforge-baseline.json --gate new --reproducible
```

`--gate all` fails on every current policy Issue. Most rules remain preview/off;
enable selected preview rules as advisories in versioned `reforge.toml`. Only
stable rules can be enforced as policy.

## Configuration and rules

`reforge init` writes a versioned configuration. Use `reforge config validate`, `reforge config show`, and `reforge rules --output json` to inspect effective settings and rule contracts. Durable settings belong in `reforge.toml`; temporary overrides use `--set key=value`.

## Troubleshooting

Use Coverage and its capability limitations when zero Issues are reported. Regenerate incompatible older reports rather than editing them. If a Dataflow policy is rejected, verify that it names one supported language and that every source and sink path/symbol names exactly one frontend declaration.
