# Auditable refactoring guardrails for agent-written code

Reforge is a local analyzer and CI gate for structural risk in human- and agent-written repositories. Every decision unit is a typed Issue backed by explicit Evidence; every report also states Coverage, limitations, and suppressions.

It is designed to answer three questions:

1. What refactoring risk was observed?
2. What measurements, locations, or exact value-flow witness support it?
3. What could the selected analyses not observe?

Reforge does not upload code, collect telemetry, or emit a health, severity, priority, or defect-probability score.

## Quick start

Unix:

```sh
curl -fsSL https://raw.githubusercontent.com/LyleMi/Reforge/main/scripts/install.sh | sh
reforge analyze .
```

PowerShell:

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/LyleMi/Reforge/main/scripts/install.ps1)))
reforge analyze .
```

Codebase runs by default. Select Dataflow explicitly, or pass both analyses to reuse one workspace index:

```sh
reforge analyze . --analysis dataflow --output json --reproducible
reforge analyze . --analysis codebase --analysis dataflow --reproducible
```

View the [live self-analysis](sample/) or continue with the [User Guide](user-guide.md).

## Documentation map

- [Analyses](analyses.md), [Dataflow](dataflow.md), and [rule cards](rule-cards.md)
- [Configuration](configuration.md) and [schema 27](report-schema.md)
- [HTML report](report-app.md) and [agent workflows](agent-workflows.md)
- [Upgrading from 0.1 to 0.2](upgrading-to-0.2.md)
- [Architecture](architecture.md), [calibration](calibration-samples.md), and [release](release.md)

Codebase supports Rust, JavaScript, TypeScript/TSX, Vue SFC script blocks, Python, Go, Java, C#, Kotlin, PHP, Ruby, Bash, and PowerShell. Dataflow currently builds exact conservative paths for Rust, JavaScript/TypeScript/TSX, and Python.
