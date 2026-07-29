# Dataflow

Dataflow builds a language-neutral Flow IR for Rust,
JavaScript/TypeScript/TSX, and Python. Selecting Dataflow records internal
observations and capability receipts; enabled preview rules can surface
advisories, while configured policies add exact bypass evaluation.

Coverage retains every language discovered in the shared workspace index.
Rust, JavaScript, TypeScript, TSX, and Python receive rule observations;
other languages are explicitly `unsupported`. Parse failures, unresolved
edges, path truncation, and missing policy configuration use stable
language/rule limitation codes and explicit capability receipts.

## Preview rules

- `reforge.dataflow.excessive_relay`, when enabled, requires an exact complete path meeting all
  three inclusive relay minima: function hops, module hops, and relay percent.
- `reforge.dataflow.flow_fan_out`, when enabled, groups by source symbol and requires both the
  distinct sink-symbol and module minima.
- `reforge.dataflow.adapter_flow_bypass`, when enabled, requires an explicit policy and an
  exact complete witness that bypasses its adapter.

All three are `preview`, default off, and advisory-only. Same-module
forwarding, modeled or unresolved paths, unsupported semantics, generated or
test sources, and truncated searches do not produce these Issues.

## Search and signal thresholds

Search budgets bound deterministic traversal under `[dataflow.search]`:
`max-path-steps`, `max-function-hops`, `max-module-hops`,
`max-paths-per-source`, `max-sinks-per-source`, and `work-budget`.

Signal thresholds live separately under `[dataflow.relay]` and
`[dataflow.fan-out]`. Changing a search budget never changes the rule claim.

Treat zero Issues together with coverage. `partial`, `unsupported`, and stable
limitation codes identify where absence is not evidence.
