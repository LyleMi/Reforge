#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_covers_every_executable_rule_once() {
        let registry = rule_registry();
        assert_eq!(registry.len(), Rule::ALL.len());
        let mut kinds = registry.iter().map(|entry| entry.kind).collect::<Vec<_>>();
        kinds.sort();
        kinds.dedup();
        assert_eq!(kinds.as_slice(), Rule::ALL);
        assert_eq!(
            registry
                .iter()
                .map(|entry| entry.analysis.as_str())
                .collect::<std::collections::BTreeSet<_>>(),
            std::collections::BTreeSet::from(["codebase", "dataflow"])
        );
        assert!(registry.iter().all(|entry| {
            !entry.rule.is_empty()
                && !entry.description.is_empty()
                && !entry.family.guidance().is_empty()
                && !entry.languages.is_empty()
        }));
    }

    #[test]
    fn rules_and_families_are_unique_within_their_contracts() {
        let registry = rule_registry();
        let rules = registry
            .iter()
            .map(|entry| entry.rule.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(rules.len(), registry.len());

        let families = registry
            .iter()
            .map(|entry| entry.family.id())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(families.len(), 18);
    }

    #[test]
    fn readability_rules_share_family_and_subject_kind() {
        let long_function = rule_spec(Rule::LongFunction);
        assert_eq!(long_function.subject, SubjectKind::Symbol);
        assert_eq!(
            long_function.family,
            IssueFamily::FunctionReadability
        );
    }

    #[test]
    fn rule_specs_have_unique_typed_measurements() {
        for entry in rule_registry() {
            let mut measurements = entry.measurements.clone();
            measurements.sort_unstable();
            measurements.dedup();
            assert_eq!(
                measurements.len(),
                entry.measurements.len(),
                "{:?}",
                entry.kind
            );
        }
    }

    #[test]
    fn script_languages_are_scoped_to_parsed_detectors() {
        let long_function = rule_spec(Rule::LongFunction);
        assert!(long_function.languages.contains(&"bash".to_string()));
        assert!(long_function.languages.contains(&"powershell".to_string()));

        for kind in [
            Rule::UnusedFunction,
            Rule::DependencyCycle,
            Rule::DependencyHub,
        ] {
            let entry = rule_spec(kind);
            assert!(!entry.languages.contains(&"bash".to_string()));
            assert!(!entry.languages.contains(&"powershell".to_string()));
        }
    }
}
