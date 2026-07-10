use crate::model::{
    DetectionApproach, DetectorManifestEntry, DetectorRelation, DetectorRelationKind, EntityScope,
    FindingKind, PrecisionRisk, QualityConstruct, RefactorAction, SignalMechanism,
};

mod raw_metrics;

pub(crate) use raw_metrics::raw_metric_manifest;

const ALL_FINDING_KINDS: &[FindingKind] = &[
    FindingKind::LargeFile,
    FindingKind::LargeDirectory,
    FindingKind::DebtMarker,
    FindingKind::SimilarFunctions,
    FindingKind::LongFunction,
    FindingKind::ComplexFunction,
    FindingKind::DeepNesting,
    FindingKind::ManyParameters,
    FindingKind::ReadabilityRisk,
    FindingKind::LargeType,
    FindingKind::LargePublicSurface,
    FindingKind::ImportHeavyFile,
    FindingKind::FunctionProliferation,
    FindingKind::UnusedFunction,
    FindingKind::RepeatedLiteral,
    FindingKind::RepeatedErrorPattern,
    FindingKind::TestDuplication,
    FindingKind::HappyPathOnlyTests,
    FindingKind::FileNamingDrift,
    FindingKind::DirectoryDrift,
    FindingKind::DataClump,
    FindingKind::ParallelImplementation,
    FindingKind::ShadowedAbstraction,
    FindingKind::DuplicateTypeShape,
    FindingKind::ConfigKeyDrift,
    FindingKind::FixtureFactoryDrift,
    FindingKind::GenericBucketDrift,
    FindingKind::AdapterBoundaryBypass,
    FindingKind::StaleCompatibilityPath,
    FindingKind::MissingDocumentationSet,
    FindingKind::MissingUserGuide,
    FindingKind::MissingReportSchemaDocs,
    FindingKind::MissingMetricsModelDocs,
    FindingKind::MissingArchitectureDocs,
    FindingKind::StaleCliDocumentation,
    FindingKind::StaleSchemaDocumentation,
    FindingKind::DependencyCycle,
    FindingKind::DependencyHub,
];

pub(crate) fn classification(kind: FindingKind) -> (QualityConstruct, SignalMechanism) {
    use FindingKind as K;
    use QualityConstruct as C;
    use SignalMechanism as M;

    match kind {
        K::DependencyCycle | K::DependencyHub | K::ImportHeavyFile | K::LargePublicSurface => {
            (C::Modularity, M::DependencyPropagation)
        }
        K::AdapterBoundaryBypass => (C::Modularity, M::DependencyPropagation),
        K::SimilarFunctions
        | K::RepeatedLiteral
        | K::RepeatedErrorPattern
        | K::DataClump
        | K::ParallelImplementation
        | K::ShadowedAbstraction
        | K::DuplicateTypeShape
        | K::ConfigKeyDrift => (C::Reusability, M::DuplicationDivergence),
        K::TestDuplication | K::FixtureFactoryDrift => (C::Testability, M::DuplicationDivergence),
        K::HappyPathOnlyTests => (C::Testability, M::VerificationDifficulty),
        K::LongFunction
        | K::ComplexFunction
        | K::DeepNesting
        | K::ManyParameters
        | K::ReadabilityRisk => (C::Analysability, M::CognitiveLoad),
        K::LargeFile
        | K::LargeDirectory
        | K::LargeType
        | K::FunctionProliferation
        | K::UnusedFunction
        | K::DirectoryDrift
        | K::GenericBucketDrift => (C::Modifiability, M::ResponsibilityDispersion),
        K::DebtMarker | K::StaleCompatibilityPath => (C::Modifiability, M::ChangePressure),
        K::FileNamingDrift
        | K::MissingDocumentationSet
        | K::MissingUserGuide
        | K::MissingReportSchemaDocs
        | K::MissingMetricsModelDocs
        | K::MissingArchitectureDocs
        | K::StaleCliDocumentation
        | K::StaleSchemaDocumentation => (C::Analysability, M::KnowledgeDrift),
    }
}

pub(crate) fn detector_manifest() -> Vec<DetectorManifestEntry> {
    ALL_FINDING_KINDS
        .iter()
        .copied()
        .map(|kind| {
            let (construct, mechanism) = classification(kind);
            DetectorManifestEntry {
                kind,
                construct,
                mechanism,
                action: action(kind),
                entity_scope: entity_scope(kind),
                approach: approach(kind),
                supported_languages: supported_languages(kind)
                    .iter()
                    .map(|language| (*language).to_string())
                    .collect(),
                precision_risk: precision_risk(kind),
                parent_kind: parent_kind(kind),
                relations: relations(kind).to_vec(),
            }
        })
        .collect()
}

pub(crate) fn action(kind: FindingKind) -> RefactorAction {
    use FindingKind as K;
    use RefactorAction as A;

    match kind {
        K::LongFunction
        | K::ComplexFunction
        | K::DeepNesting
        | K::ManyParameters
        | K::ReadabilityRisk => A::SimplifyFunction,
        K::DependencyCycle
        | K::DependencyHub
        | K::ImportHeavyFile
        | K::LargePublicSurface
        | K::AdapterBoundaryBypass => A::ReduceDependencyCoupling,
        K::LargeFile
        | K::LargeDirectory
        | K::LargeType
        | K::FunctionProliferation
        | K::DirectoryDrift
        | K::GenericBucketDrift => A::DecomposeResponsibility,
        K::SimilarFunctions
        | K::RepeatedLiteral
        | K::RepeatedErrorPattern
        | K::DataClump
        | K::ParallelImplementation
        | K::ShadowedAbstraction
        | K::DuplicateTypeShape
        | K::ConfigKeyDrift => A::ConsolidateDuplication,
        K::TestDuplication | K::FixtureFactoryDrift => A::ConsolidateTestSupport,
        K::HappyPathOnlyTests => A::StrengthenTestCoverage,
        K::UnusedFunction => A::RemoveDeadCode,
        K::DebtMarker => A::ResolveDeclaredDebt,
        K::FileNamingDrift => A::StandardizeNaming,
        K::StaleCompatibilityPath => A::RetireCompatibility,
        K::MissingDocumentationSet
        | K::MissingUserGuide
        | K::MissingReportSchemaDocs
        | K::MissingMetricsModelDocs
        | K::MissingArchitectureDocs
        | K::StaleCliDocumentation
        | K::StaleSchemaDocumentation => A::RestoreDocumentation,
    }
}

fn entity_scope(kind: FindingKind) -> EntityScope {
    use EntityScope as E;
    use FindingKind as K;

    match kind {
        K::MissingDocumentationSet
        | K::MissingUserGuide
        | K::MissingReportSchemaDocs
        | K::MissingMetricsModelDocs
        | K::MissingArchitectureDocs
        | K::StaleCliDocumentation
        | K::StaleSchemaDocumentation => E::Repository,
        K::LargeDirectory | K::FileNamingDrift | K::DirectoryDrift | K::GenericBucketDrift => {
            E::Directory
        }
        K::LargeFile
        | K::DebtMarker
        | K::LargePublicSurface
        | K::ImportHeavyFile
        | K::FunctionProliferation
        | K::UnusedFunction
        | K::DependencyHub => E::File,
        K::LongFunction
        | K::ComplexFunction
        | K::DeepNesting
        | K::ManyParameters
        | K::ReadabilityRisk => E::Function,
        K::LargeType => E::Type,
        K::SimilarFunctions
        | K::RepeatedLiteral
        | K::RepeatedErrorPattern
        | K::TestDuplication
        | K::HappyPathOnlyTests
        | K::DataClump
        | K::ParallelImplementation
        | K::ShadowedAbstraction
        | K::DuplicateTypeShape
        | K::ConfigKeyDrift
        | K::FixtureFactoryDrift
        | K::AdapterBoundaryBypass
        | K::StaleCompatibilityPath
        | K::DependencyCycle => E::FindingGroup,
    }
}

fn approach(kind: FindingKind) -> DetectionApproach {
    use DetectionApproach as A;
    use FindingKind as K;

    match kind {
        K::LargeFile
        | K::LargeDirectory
        | K::LongFunction
        | K::ComplexFunction
        | K::DeepNesting
        | K::ManyParameters
        | K::ReadabilityRisk
        | K::LargeType
        | K::LargePublicSurface
        | K::ImportHeavyFile
        | K::FunctionProliferation => A::Threshold,
        K::DependencyCycle | K::DependencyHub => A::GraphAnalysis,
        K::MissingDocumentationSet
        | K::MissingUserGuide
        | K::MissingReportSchemaDocs
        | K::MissingMetricsModelDocs
        | K::MissingArchitectureDocs
        | K::StaleCliDocumentation
        | K::StaleSchemaDocumentation => A::RepositoryAudit,
        K::SimilarFunctions
        | K::UnusedFunction
        | K::RepeatedLiteral
        | K::RepeatedErrorPattern
        | K::TestDuplication
        | K::HappyPathOnlyTests
        | K::DataClump
        | K::DuplicateTypeShape => A::ParsedAnalysis,
        K::DebtMarker
        | K::FileNamingDrift
        | K::DirectoryDrift
        | K::ParallelImplementation
        | K::ShadowedAbstraction
        | K::ConfigKeyDrift
        | K::FixtureFactoryDrift
        | K::GenericBucketDrift
        | K::AdapterBoundaryBypass
        | K::StaleCompatibilityPath => A::Heuristic,
    }
}

fn precision_risk(kind: FindingKind) -> PrecisionRisk {
    use FindingKind as K;
    use PrecisionRisk as R;

    if matches!(
        kind,
        K::LargeFile
            | K::LargeDirectory
            | K::LongFunction
            | K::ComplexFunction
            | K::DeepNesting
            | K::ManyParameters
            | K::LargeType
            | K::LargePublicSurface
            | K::ImportHeavyFile
            | K::DependencyCycle
    ) {
        R::Low
    } else if matches!(
        kind,
        K::ReadabilityRisk
            | K::SimilarFunctions
            | K::RepeatedErrorPattern
            | K::TestDuplication
            | K::DataClump
            | K::DuplicateTypeShape
            | K::ConfigKeyDrift
            | K::FixtureFactoryDrift
            | K::DependencyHub
            | K::MissingDocumentationSet
            | K::MissingUserGuide
            | K::MissingReportSchemaDocs
            | K::MissingMetricsModelDocs
            | K::MissingArchitectureDocs
            | K::StaleCliDocumentation
            | K::StaleSchemaDocumentation
    ) {
        R::Medium
    } else {
        R::High
    }
}

pub(crate) fn parent_kind(kind: FindingKind) -> Option<FindingKind> {
    use FindingKind as K;

    match kind {
        K::LongFunction | K::ComplexFunction | K::DeepNesting | K::ManyParameters => {
            Some(K::ReadabilityRisk)
        }
        K::MissingUserGuide
        | K::MissingReportSchemaDocs
        | K::MissingMetricsModelDocs
        | K::MissingArchitectureDocs => Some(K::MissingDocumentationSet),
        _ => None,
    }
}

const fn alternative_evidence(kind: FindingKind) -> DetectorRelation {
    DetectorRelation {
        kind,
        relation: DetectorRelationKind::AlternativeEvidence,
    }
}

const fn facet_of(kind: FindingKind) -> DetectorRelation {
    DetectorRelation {
        kind,
        relation: DetectorRelationKind::FacetOf,
    }
}

const READABILITY_FACET: &[DetectorRelation] = &[facet_of(FindingKind::ReadabilityRisk)];
const DOCUMENTATION_FACET: &[DetectorRelation] = &[facet_of(FindingKind::MissingDocumentationSet)];
const SIMILAR_RELATIONS: &[DetectorRelation] = &[
    alternative_evidence(FindingKind::ParallelImplementation),
    alternative_evidence(FindingKind::ShadowedAbstraction),
];
const PARALLEL_RELATIONS: &[DetectorRelation] = &[
    alternative_evidence(FindingKind::SimilarFunctions),
    alternative_evidence(FindingKind::ShadowedAbstraction),
];
const SHADOWED_RELATIONS: &[DetectorRelation] = &[
    alternative_evidence(FindingKind::SimilarFunctions),
    alternative_evidence(FindingKind::ParallelImplementation),
];
const REPEATED_LITERAL_RELATIONS: &[DetectorRelation] =
    &[alternative_evidence(FindingKind::ConfigKeyDrift)];
const CONFIG_KEY_RELATIONS: &[DetectorRelation] =
    &[alternative_evidence(FindingKind::RepeatedLiteral)];
const TEST_DUPLICATION_RELATIONS: &[DetectorRelation] =
    &[alternative_evidence(FindingKind::FixtureFactoryDrift)];
const FIXTURE_RELATIONS: &[DetectorRelation] =
    &[alternative_evidence(FindingKind::TestDuplication)];
const LARGE_DIRECTORY_RELATIONS: &[DetectorRelation] = &[
    alternative_evidence(FindingKind::DirectoryDrift),
    alternative_evidence(FindingKind::GenericBucketDrift),
];
const DIRECTORY_DRIFT_RELATIONS: &[DetectorRelation] = &[
    alternative_evidence(FindingKind::LargeDirectory),
    alternative_evidence(FindingKind::GenericBucketDrift),
];
const GENERIC_BUCKET_RELATIONS: &[DetectorRelation] = &[
    alternative_evidence(FindingKind::LargeDirectory),
    alternative_evidence(FindingKind::DirectoryDrift),
];

pub(crate) fn relations(kind: FindingKind) -> &'static [DetectorRelation] {
    use FindingKind as K;

    match kind {
        K::LongFunction | K::ComplexFunction | K::DeepNesting | K::ManyParameters => {
            READABILITY_FACET
        }
        K::MissingUserGuide
        | K::MissingReportSchemaDocs
        | K::MissingMetricsModelDocs
        | K::MissingArchitectureDocs => DOCUMENTATION_FACET,
        K::SimilarFunctions => SIMILAR_RELATIONS,
        K::ParallelImplementation => PARALLEL_RELATIONS,
        K::ShadowedAbstraction => SHADOWED_RELATIONS,
        K::RepeatedLiteral => REPEATED_LITERAL_RELATIONS,
        K::ConfigKeyDrift => CONFIG_KEY_RELATIONS,
        K::TestDuplication => TEST_DUPLICATION_RELATIONS,
        K::FixtureFactoryDrift => FIXTURE_RELATIONS,
        K::LargeDirectory => LARGE_DIRECTORY_RELATIONS,
        K::DirectoryDrift => DIRECTORY_DRIFT_RELATIONS,
        K::GenericBucketDrift => GENERIC_BUCKET_RELATIONS,
        _ => &[],
    }
}

fn supported_languages(kind: FindingKind) -> &'static [&'static str] {
    use FindingKind as K;

    const ALL_PARSED: &[&str] = &[
        "rust",
        "javascript",
        "typescript",
        "tsx",
        "python",
        "go",
        "java",
        "csharp",
        "kotlin",
        "php",
        "ruby",
    ];
    const UNUSED: &[&str] = &["rust", "javascript", "typescript", "tsx", "python", "go"];
    const GRAPH: &[&str] = &[
        "rust",
        "javascript",
        "typescript",
        "tsx",
        "python",
        "ruby",
        "c",
        "cpp",
    ];
    const REPOSITORY: &[&str] = &["repository"];
    const PATHS: &[&str] = &["language_neutral_paths"];

    match kind {
        K::MissingDocumentationSet
        | K::MissingUserGuide
        | K::MissingReportSchemaDocs
        | K::MissingMetricsModelDocs
        | K::MissingArchitectureDocs
        | K::StaleCliDocumentation
        | K::StaleSchemaDocumentation => REPOSITORY,
        K::LargeFile | K::LargeDirectory | K::DebtMarker | K::FileNamingDrift => PATHS,
        K::UnusedFunction => UNUSED,
        K::DependencyCycle | K::DependencyHub => GRAPH,
        _ => ALL_PARSED,
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{MetricDirection, MetricScale};

    use super::*;

    #[test]
    fn manifest_covers_every_finding_kind_once() {
        let manifest = detector_manifest();
        assert_eq!(manifest.len(), ALL_FINDING_KINDS.len());
        let mut kinds = manifest.iter().map(|entry| entry.kind).collect::<Vec<_>>();
        kinds.sort();
        kinds.dedup();
        assert_eq!(kinds.len(), ALL_FINDING_KINDS.len());
    }

    #[test]
    fn atomic_readability_signals_name_their_parent() {
        let manifest = detector_manifest();
        let long_function = manifest
            .iter()
            .find(|entry| entry.kind == FindingKind::LongFunction)
            .unwrap();
        assert_eq!(
            long_function.parent_kind,
            Some(FindingKind::ReadabilityRisk)
        );
        assert_eq!(long_function.mechanism, SignalMechanism::CognitiveLoad);
        assert_eq!(long_function.action, RefactorAction::SimplifyFunction);
        assert_eq!(long_function.entity_scope, EntityScope::Function);
    }

    #[test]
    fn facet_relations_match_declared_parents() {
        for entry in detector_manifest() {
            let facet_relations = entry
                .relations
                .iter()
                .filter(|relation| relation.relation == DetectorRelationKind::FacetOf)
                .collect::<Vec<_>>();

            for relation in &facet_relations {
                assert_eq!(entry.parent_kind, Some(relation.kind));
                assert_eq!(entry.action, action(relation.kind));
            }

            match entry.parent_kind {
                Some(parent) => assert_eq!(
                    facet_relations.as_slice(),
                    [&DetectorRelation {
                        kind: parent,
                        relation: DetectorRelationKind::FacetOf,
                    }]
                ),
                None => assert!(facet_relations.is_empty()),
            }
        }
    }

    #[test]
    fn alternative_evidence_relations_are_reciprocal() {
        let manifest = detector_manifest();
        for entry in &manifest {
            for relation in entry
                .relations
                .iter()
                .filter(|relation| relation.relation == DetectorRelationKind::AlternativeEvidence)
            {
                let reciprocal = manifest
                    .iter()
                    .find(|candidate| candidate.kind == relation.kind)
                    .unwrap();
                assert!(reciprocal.relations.iter().any(|candidate| {
                    candidate.kind == entry.kind
                        && candidate.relation == DetectorRelationKind::AlternativeEvidence
                }));
                assert_eq!(entry.action, reciprocal.action);
            }
        }
    }

    #[test]
    fn raw_metric_manifest_has_unique_names_and_explicit_context_metrics() {
        let manifest = raw_metric_manifest();
        let mut names = manifest
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>();
        names.sort_unstable();
        names.dedup();

        assert_eq!(names.len(), manifest.len());
        assert!(manifest.iter().any(|entry| {
            entry.name == "file.is_test"
                && entry.scale == MetricScale::Boolean
                && entry.direction == MetricDirection::ContextOnly
        }));
    }
}
