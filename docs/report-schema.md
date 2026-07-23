# Report schema 26

The public Rust type is `reforge_schema::Report`. The top-level fields are `schema_version`, `producer`, `target`, `summary`, `suppression`, `coverage`, `issues`, and optional `baseline_comparison`.

An Issue contains `id`, explicit `analysis`, `family`, canonical `subject`, readable `title`, `guidance`, and Evidence. `analysis` must name a key in the report's Coverage map; consumers never infer ownership from a family prefix. Its `ri6-*` ID hashes only family and subject, so adding or reordering Evidence does not change Issue identity.

Evidence contains `id`, `rule`, `message`, measurements, locations, and an optional typed Flow witness. Its `re6-*` ID hashes only rule and semantic anchor. Flow witnesses expose source and sink symbols, ordered steps, hop counts, and resolution; internal graph node IDs are not report identity or display text.

Coverage is keyed by analysis. Each entry records overall status, actual scanned files, and cross-rule limitations. Every discovered language has its own status, file/function counts, and limitations; unsupported Dataflow languages remain visible. Rule execution contains one or more named observations with explicit count and unit, plus rule-local limitations. Zero Evidence therefore does not erase the scanned denominator.

Reports never contain raw metrics, Flow IR, arbitrary extensions, or internal ontology fields. Unknown fields and older or transitional schemas are rejected.

Schema 26's wire shape was tightened before release. Regenerate reports and baselines that used the earlier schema 26 migration shape.
