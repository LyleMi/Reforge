---
name: reforge-analyze
description: Run the unified Reforge Codebase/Dataflow analyzer and explain schema 26 Issue, Evidence, witness, suppression, and coverage contracts.
---

# Reforge Analyze

Generated contract: CLI `0.2.0`, report schema `26`, artifact schema `5`.

1. Check `reforge --version`; stop on a version mismatch.
2. Omit `--analysis` for Codebase, choose `--analysis dataflow` for Dataflow alone, or pass both `--analysis codebase --analysis dataflow` for one combined report.
3. Run `reforge analyze <root> --output json --output-file <report>.json --reproducible`. Request raw metrics or the complete Flow IR only through `--metrics-output` or `--flow-ir-output`.
4. Treat `issues` as the only decision units and read their nested Evidence. For Dataflow, preserve the source-to-sink order of the witness and never present a partial/unresolved path as exact.
5. Read every selected analysis coverage status, language count, limitation, rule execution, and suppression count before interpreting absence. Empty Issues is an observed zero only for observable coverage.
6. Use `reforge rules --output json` for rule ownership, descriptions, languages, default state, and measurements. Put durable settings in versioned `reforge.toml`; temporary overrides use dotted `--set key=value`.
7. Do not add suppressions, alter thresholds, install dependencies, edit source, or publish changes unless the user asks.
