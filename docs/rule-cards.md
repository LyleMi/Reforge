# Rule cards

Every core rule is a refactoring-inspection claim, not a defect prediction,
health score, generic priority, or automatic architecture inference. All cards
inherit these non-goals. A finding's identity comes from its typed subject and
the rule-specific semantic anchor, not prose, ordering, checkout location, or
line numbers. Measurement, threshold, evidence-set, or witness changes update
the finding's content fingerprint.

All rules below are currently `preview`, `default_enabled = false`,
`validation_basis = fixture`, semantic version `1.0.0`, and ineligible for
enforcement. A language can become stable only through the audited calibration
protocol in `calibration/README.md`; other languages remain preview.

| Rule | Claim / inspection question | Capability | Positive and negative fixtures | Legitimate exceptions |
| --- | --- | --- | --- | --- |
| `reforge.codebase.large_file` | A file exceeds the configured line boundary; is responsibility ownership too broad? | file inventory | over/under threshold | generated facades, declarative tables |
| `reforge.codebase.large_directory` | A directory owns more direct source files than configured. | directory inventory | wide/narrow directories | flat packages with explicit ownership |
| `reforge.codebase.debt_marker` | A source comment explicitly declares TODO/FIXME debt. | source text | comment/non-comment markers | generated or externally tracked markers |
| `reforge.codebase.similar_functions` | Multiple normalized bodies are structurally similar enough to inspect together. | parsed syntax similarity | cloned/distinct bodies | protocol implementations, tests |
| `reforge.codebase.long_function` | A declared function exceeds the configured line span. | syntax and symbols | long/short functions | generated parsers, linear tables |
| `reforge.codebase.complex_function` | Estimated branch complexity exceeds the configured bound. | parsed control syntax | branch-heavy/linear functions | explicit state machines |
| `reforge.codebase.deep_nesting` | Lexical control nesting exceeds the configured bound. | parsed control syntax | nested/guard-clause fixtures | recursive walkers |
| `reforge.codebase.many_parameters` | A function declares more parameters than configured. | symbol parameters | over/under arity | serialization and FFI boundaries |
| `reforge.codebase.large_type` | A type exceeds configured span or member count. | type observations | large/small declarations | generated schemas |
| `reforge.codebase.large_public_surface` | A file exports more items than configured. | export syntax | broad/narrow modules | deliberate prelude or facade |
| `reforge.codebase.import_heavy_file` | A file imports more dependencies than configured. | import syntax | over/under import count | composition roots |
| `reforge.codebase.function_proliferation` | A file combines high function count, density, and small-function ratio. | function inventory | dense/sparse files | parser combinators |
| `reforge.codebase.low_module_cohesion` | Multiple exact call-connected function clusters suggest separable module responsibilities. | module function call graph | monolith/split modules | routers, registries, controllers |
| `reforge.codebase.unused_function` | A private symbol has no supported project-local reference. | symbols and references | referenced/unreferenced symbols | reflection, callbacks, macros |
| `reforge.codebase.repeated_literal` | A literal repeats enough to inspect ownership. | parsed literals | repeated/unique literals | protocol constants and test data |
| `reforge.codebase.repeated_error_pattern` | Error-handling syntax repeats across sites. | parsed error syntax | repeated/distinct handlers | intentionally local recovery |
| `reforge.codebase.test_duplication` | Test setup patterns repeat across tests. | parsed test syntax | duplicated/distinct setup | readability-focused local setup |
| `reforge.codebase.happy_path_only_tests` | A test group has assertions without detected failure/boundary cases. | test syntax | positive-only/mixed tests | behavior proven elsewhere |
| `reforge.codebase.file_naming_drift` | A directory mixes file naming conventions. | path inventory | mixed/uniform names | language-required names |
| `reforge.codebase.directory_drift` | Directory concepts exceed the configured ownership bound. | paths and syntax names | mixed/cohesive fixtures | plugin registries |
| `reforge.codebase.data_clump` | The same parameter combination recurs across functions. | symbol parameters | recurring/distinct sets | stable protocol signatures |
| `reforge.codebase.parallel_implementation` | Similarly named capabilities are implemented independently. | symbol concepts | parallel/unrelated names | platform-specific variants |
| `reforge.codebase.shadowed_abstraction` | Local helpers overlap a shared abstraction. | symbols and concepts | local/shared overlap | deliberate compatibility shims |
| `reforge.codebase.duplicate_type_shape` | Type field shapes substantially overlap. | type fields | overlapping/distinct shapes | boundary DTOs |
| `reforge.codebase.config_key_drift` | Configuration-like keys repeat or drift. | literal concepts | repeated/distinct keys | external protocol keys |
| `reforge.codebase.fixture_factory_drift` | Test fixture/factory concepts repeat independently. | test symbols | duplicated/distinct factories | domain-specific builders |
| `reforge.codebase.generic_bucket_drift` | A generic directory or file accumulates unrelated concepts. | typed file/directory subjects | generic/cohesive buckets | intentionally tiny shared kernels |
| `reforge.codebase.adapter_boundary_bypass` | Naming/syntax suggests direct access around an adapter. | heuristic concepts | bypass/non-bypass fixtures | migration and bootstrap code |
| `reforge.codebase.stale_compatibility_path` | Compatibility markers lack an explicit retirement boundary. | parsed compatibility syntax | stale/owned paths | supported long-term compatibility |
| `reforge.codebase.dependency_cycle` | Resolved project-local dependencies form a cycle. | dependency graph | cyclic/acyclic graphs | mutually recursive generated modules |
| `reforge.codebase.dependency_hub` | A file has unusually broad/deep resolved dependency topology. | dependency graph | hub/leaf graphs | composition roots and public facades |
| `reforge.dataflow.adapter_flow_bypass` | An exact, value-preserving path violates a complete single-language adapter policy. | exact local/interprocedural flow | exact bypass/conforming and unsupported fixtures | explicit exemptions |
| `reforge.dataflow.excessive_relay` | An exact path contains configured forwarding depth; inspect ownership only. | exact direct-call flow | long/short relay paths | pipelines, middleware, telemetry |
| `reforge.dataflow.flow_fan_out` | One exact source reaches many supported sinks/modules. | exact direct-call flow | fan-out/narrow paths | orchestrators and event distribution |

Similarity, literal, generic-bucket, unused-function, adapter, relay, and
fan-out heuristics remain preview/off until each language independently meets
the calibration gates. Self-scan is regression data only and cannot promote a
rule or select a threshold.
