# Dataflow

Dataflow builds a language-neutral Flow IR for Rust,
JavaScript/TypeScript/TSX, and Python. General rules always run when Dataflow is
selected; configured policies add policy-bypass evaluation.

Coverage retains every language discovered in the shared workspace index.
Rust, JavaScript, TypeScript, TSX, and Python receive rule observations;
other languages are explicitly `unsupported`. Parse failures, unresolved
edges, path truncation, and missing policy configuration use stable
language/rule limitation codes rather than an internal capability matrix.

## Stable rules

- `reforge.dataflow.excessive_relay` requires an exact complete path meeting all
  three inclusive relay minima: function hops, module hops, and relay percent.
- `reforge.dataflow.flow_fan_out` groups by source symbol and requires both the
  distinct sink-symbol and module minima.
- `reforge.dataflow.adapter_flow_bypass` requires an explicit policy and an
  exact complete witness that bypasses its adapter.

Same-module forwarding, unresolved paths, unsupported semantics, generated or
test sources, and truncated searches do not produce these Issues.

## Search and signal thresholds

Search budgets bound deterministic traversal under `[dataflow.search]`:
`max-path-steps`, `max-function-hops`, `max-module-hops`,
`max-paths-per-source`, `max-sinks-per-source`, and `work-budget`.

Signal thresholds live separately under `[dataflow.relay]` and
`[dataflow.fan-out]`. Changing a search budget never changes the definition of
a smell.

Treat zero Issues together with coverage. `partial`, `unsupported`, and stable
limitation codes identify where absence is not evidence.
