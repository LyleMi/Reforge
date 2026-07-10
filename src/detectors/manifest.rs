use crate::model::{
    DetectionApproach, DetectorManifestEntry, FindingKind, PrecisionRisk, QualityConstruct,
    SignalMechanism,
};

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
                approach: approach(kind),
                supported_languages: supported_languages(kind)
                    .iter()
                    .map(|language| (*language).to_string())
                    .collect(),
                precision_risk: precision_risk(kind),
                parent_kind: parent_kind(kind),
                overlaps_with: overlaps_with(kind),
            }
        })
        .collect()
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

    match kind {
        K::LargeFile
        | K::LargeDirectory
        | K::LongFunction
        | K::ComplexFunction
        | K::DeepNesting
        | K::ManyParameters
        | K::LargeType
        | K::LargePublicSurface
        | K::ImportHeavyFile
        | K::DependencyCycle => R::Low,
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
        | K::StaleSchemaDocumentation => R::Medium,
        K::DebtMarker
        | K::FunctionProliferation
        | K::UnusedFunction
        | K::RepeatedLiteral
        | K::HappyPathOnlyTests
        | K::FileNamingDrift
        | K::DirectoryDrift
        | K::ParallelImplementation
        | K::ShadowedAbstraction
        | K::GenericBucketDrift
        | K::AdapterBoundaryBypass
        | K::StaleCompatibilityPath => R::High,
    }
}

fn parent_kind(kind: FindingKind) -> Option<FindingKind> {
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

fn overlaps_with(kind: FindingKind) -> Vec<FindingKind> {
    use FindingKind as K;

    match kind {
        K::SimilarFunctions => vec![K::ParallelImplementation, K::ShadowedAbstraction],
        K::ParallelImplementation | K::ShadowedAbstraction => vec![K::SimilarFunctions],
        K::RepeatedLiteral => vec![K::ConfigKeyDrift],
        K::ConfigKeyDrift => vec![K::RepeatedLiteral],
        K::TestDuplication => vec![K::FixtureFactoryDrift],
        K::FixtureFactoryDrift => vec![K::TestDuplication],
        K::LargeDirectory => vec![K::DirectoryDrift, K::GenericBucketDrift],
        K::DirectoryDrift | K::GenericBucketDrift => vec![K::LargeDirectory],
        _ => Vec::new(),
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
    }
}
