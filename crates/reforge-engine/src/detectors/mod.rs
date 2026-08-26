pub(crate) mod concepts;
pub(crate) mod data_flow;
pub(crate) mod dependency_graph;
pub(crate) mod drift;
pub(crate) mod manifest;
pub(crate) mod similarity;
pub(crate) mod structure;
pub(crate) mod unused_functions;

#[cfg(test)]
mod repository_contract_tests {
    const USER_GUIDE: &str = include_str!("../../../../docs/user-guide.md");
    const REPORT_SCHEMA: &str = include_str!("../../../../docs/report-schema.md");
    const METRICS_MODEL: &str = include_str!("../../../../docs/metrics-model.md");
    const ARCHITECTURE: &str = include_str!("../../../../docs/architecture.md");

    #[test]
    fn user_guide_covers_the_public_operating_contract() {
        for topic in ["install", "analyze", "output", "troubleshoot"] {
            assert!(
                USER_GUIDE.to_ascii_lowercase().contains(topic),
                "user guide must cover {topic}"
            );
        }
    }

    #[test]
    fn report_documentation_tracks_schema_27_fields() {
        for field in [
            "schema_version",
            "provenance",
            "content_fingerprint",
            "baseline_comparison",
            "advisory",
            "policy",
        ] {
            assert!(
                REPORT_SCHEMA.contains(field),
                "report schema documentation must cover {field}"
            );
        }
    }

    #[test]
    fn metrics_and_architecture_contracts_are_present() {
        let metrics = METRICS_MODEL.to_ascii_lowercase();
        for term in ["Evidence", "Issue", "Coverage"] {
            assert!(metrics.contains(&term.to_ascii_lowercase()));
        }
        let architecture = ARCHITECTURE.to_ascii_lowercase();
        for term in ["Codebase", "Dataflow", "detector", "report"] {
            assert!(architecture.contains(&term.to_ascii_lowercase()));
        }
    }
}
