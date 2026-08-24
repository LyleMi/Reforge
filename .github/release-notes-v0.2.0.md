# Reforge v0.2.0

Reforge is now an auditable refactoring guardrail for agent-written code. It reports decision-ready Issues with explicit Evidence and Coverage instead of collapsing repository health into a score.

## Highlights

- One `reforge analyze` entry point for Codebase, Dataflow, or a combined shared-index analysis.
- Frozen schema 27 reports with typed subjects, stable `ri7-*` Issue IDs, `re7-*` Evidence IDs, provenance, and baseline states.
- Conservative Dataflow witnesses that preserve source-to-sink order and never present partial or unresolved paths as exact.
- Coverage receipts for every selected analysis, language, capability, rule execution, limitation, and suppression.
- Verified installers for Linux x86_64, macOS Intel and Apple Silicon, and Windows x86_64.
- Offline HTML, human, JSON, YAML, and SARIF output, plus explicit raw-metrics and Flow IR sidecars.

## Install

Unix:

```sh
curl -fsSL https://raw.githubusercontent.com/LyleMi/Reforge/v0.2.0/scripts/install.sh | sh -s -- --version v0.2.0
reforge analyze .
```

PowerShell:

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/LyleMi/Reforge/v0.2.0/scripts/install.ps1))) -Version v0.2.0
reforge analyze .
```

Both installers verify the release archive against `SHA256SUMS`, verify the binary version, and install the `reforge-analyze` Codex skill by default.

## Breaking upgrade from 0.1

The `scan` command and the separate `reforge-scan` / `reforge-flow` commands are removed. Use:

```sh
reforge analyze .
reforge analyze . --analysis dataflow
reforge analyze . --analysis codebase --analysis dataflow
```

Regenerate configuration with `reforge init`, then reapply settings using configuration version 2. Regenerate all old reports and baselines: v0.2.0 accepts schema 27 and does not claim coverage for migrated 0.1 artifacts. See the [complete upgrade guide](https://lylemi.github.io/Reforge/upgrading-to-0.2.html).

Reforge runs locally, does not upload source code, includes no telemetry, and does not publish a defect-probability score.
