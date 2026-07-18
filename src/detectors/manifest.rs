use crate::model::{
    DetectionApproach, DetectorManifestEntry, EntityScope, EvidenceRole, FindingKind, MetricId,
    PrecisionRisk, QualityConstruct, RefactorAction, SignalMechanism,
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
    FindingKind::UnityAssemblyCycle,
    FindingKind::UnityAssemblyHub,
    FindingKind::UnityUnresolvedAssemblyReference,
    FindingKind::UnityRuntimeEditorDependency,
    FindingKind::UnityDuplicateGuid,
    FindingKind::UnityMissingMeta,
    FindingKind::UnityOrphanMeta,
    FindingKind::UnityBrokenAssetReference,
    FindingKind::UnityMissingScript,
    FindingKind::UnityNonTextSerialization,
    FindingKind::UnitySceneBuildDrift,
    FindingKind::UnityLargeScene,
    FindingKind::UnityLargePrefab,
    FindingKind::UnitySerializedFieldBloat,
    FindingKind::UnityLifecycleOverload,
    FindingKind::UnityExpensiveFrameCall,
    FindingKind::UnityEditorApiInRuntime,
    FindingKind::UnityUnbalancedEventSubscription,
];

pub(crate) fn classification(kind: FindingKind) -> (QualityConstruct, SignalMechanism) {
    use FindingKind as K;
    use QualityConstruct as C;
    use SignalMechanism as M;

    match kind {
        K::DependencyCycle
        | K::DependencyHub
        | K::ImportHeavyFile
        | K::LargePublicSurface
        | K::UnityAssemblyCycle
        | K::UnityAssemblyHub
        | K::UnityUnresolvedAssemblyReference
        | K::UnityRuntimeEditorDependency
        | K::UnityBrokenAssetReference
        | K::UnityMissingScript
        | K::UnityExpensiveFrameCall
        | K::UnityEditorApiInRuntime => (C::Modularity, M::DependencyPropagation),
        K::UnityDuplicateGuid
        | K::UnityMissingMeta
        | K::UnityOrphanMeta
        | K::UnityNonTextSerialization
        | K::UnitySceneBuildDrift => (C::Analysability, M::KnowledgeDrift),
        K::UnityLargeScene
        | K::UnityLargePrefab
        | K::UnitySerializedFieldBloat
        | K::UnityLifecycleOverload => (C::Modifiability, M::ResponsibilityDispersion),
        K::UnityUnbalancedEventSubscription => (C::Testability, M::VerificationDifficulty),
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
    let manifest = ALL_FINDING_KINDS
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
                input_metrics: input_metrics(kind).to_vec(),
                issue_family: issue_family(kind).to_string(),
                evidence_role: evidence_role(kind),
                constituent_kinds: constituent_kinds(kind).to_vec(),
                default_detection_reliability: default_detection_reliability(kind),
                default_interpretation_reliability: default_interpretation_reliability(kind),
                impact: impact(kind),
                actionability: actionability(kind),
            }
        })
        .collect::<Vec<_>>();
    assert_manifest_invariants(&manifest);
    manifest
}

fn assert_manifest_invariants(manifest: &[DetectorManifestEntry]) {
    let raw = raw_metric_manifest();
    let raw_names = raw
        .iter()
        .map(|entry| entry.name)
        .collect::<std::collections::BTreeSet<_>>();
    for entry in manifest {
        assert!(
            entry
                .input_metrics
                .iter()
                .all(|metric| raw_names.contains(metric)),
            "detector {:?} declares an unknown input metric",
            entry.kind
        );
        if entry.evidence_role == EvidenceRole::CompositeSummary {
            assert!(
                !entry.constituent_kinds.is_empty(),
                "composite detector {:?} has no constituents",
                entry.kind
            );
            for constituent in &entry.constituent_kinds {
                assert_ne!(
                    *constituent, entry.kind,
                    "composite detector cycle at {:?}",
                    entry.kind
                );
                let other = manifest
                    .iter()
                    .find(|candidate| candidate.kind == *constituent)
                    .expect("composite constituent must exist");
                assert_eq!(
                    other.mechanism, entry.mechanism,
                    "composite mechanism mismatch"
                );
                assert_eq!(other.action, entry.action, "composite action mismatch");
            }
        } else {
            assert!(
                entry.constituent_kinds.is_empty(),
                "only composite detectors may declare constituents"
            );
        }
    }
}

pub(crate) fn issue_family(kind: FindingKind) -> &'static str {
    use FindingKind as K;
    match kind {
        K::LongFunction
        | K::ComplexFunction
        | K::DeepNesting
        | K::ManyParameters
        | K::ReadabilityRisk => "function_readability",
        K::MissingDocumentationSet
        | K::MissingUserGuide
        | K::MissingReportSchemaDocs
        | K::MissingMetricsModelDocs
        | K::MissingArchitectureDocs
        | K::StaleCliDocumentation
        | K::StaleSchemaDocumentation => "documentation_integrity",
        K::SimilarFunctions | K::ParallelImplementation | K::ShadowedAbstraction => {
            "implementation_duplication"
        }
        _ => match action(kind) {
            RefactorAction::SimplifyFunction => "function_readability",
            RefactorAction::ReduceDependencyCoupling => "dependency_coupling",
            RefactorAction::DecomposeResponsibility => "responsibility_decomposition",
            RefactorAction::ConsolidateDuplication => "duplication_consolidation",
            RefactorAction::ConsolidateTestSupport => "test_support_consolidation",
            RefactorAction::StrengthenTestCoverage => "test_coverage",
            RefactorAction::RemoveDeadCode => "dead_code",
            RefactorAction::ResolveDeclaredDebt => "declared_debt",
            RefactorAction::StandardizeNaming => "naming_consistency",
            RefactorAction::RetireCompatibility => "compatibility_retirement",
            RefactorAction::RestoreDocumentation => "documentation_integrity",
        },
    }
}

pub(crate) fn evidence_role(kind: FindingKind) -> EvidenceRole {
    match kind {
        FindingKind::ReadabilityRisk | FindingKind::MissingDocumentationSet => {
            EvidenceRole::CompositeSummary
        }
        FindingKind::ParallelImplementation | FindingKind::ShadowedAbstraction => {
            EvidenceRole::Alternative
        }
        _ => EvidenceRole::Atomic,
    }
}

pub(crate) fn constituent_kinds(kind: FindingKind) -> &'static [FindingKind] {
    match kind {
        FindingKind::ReadabilityRisk => &[
            FindingKind::LongFunction,
            FindingKind::ComplexFunction,
            FindingKind::DeepNesting,
            FindingKind::ManyParameters,
        ],
        FindingKind::MissingDocumentationSet => &[
            FindingKind::MissingUserGuide,
            FindingKind::MissingReportSchemaDocs,
            FindingKind::MissingMetricsModelDocs,
            FindingKind::MissingArchitectureDocs,
        ],
        _ => &[],
    }
}

pub(crate) fn input_metrics(kind: FindingKind) -> &'static [MetricId] {
    use FindingKind as K;
    use MetricId as M;

    match kind {
        K::LargeFile => &[M::FileLoc],
        K::LargeDirectory => &[M::DirectorySourceFiles],
        K::DebtMarker => &[],
        K::LongFunction => &[M::FunctionLoc],
        K::ComplexFunction => &[M::FunctionComplexity],
        K::DeepNesting => &[M::FunctionNestingDepth],
        K::ManyParameters => &[M::FunctionParameterCount],
        K::ReadabilityRisk => &[
            M::FunctionLoc,
            M::FunctionComplexity,
            M::FunctionNestingDepth,
            M::FunctionParameterCount,
            M::ReadabilitySignalCount,
        ],
        K::LargeType => &[M::TypeLoc, M::TypeMemberCount],
        K::LargePublicSurface => &[M::FilePublicItems],
        K::ImportHeavyFile => &[M::FileImports],
        K::FunctionProliferation => &[
            M::FileFunctionCount,
            M::FileFunctionsPerHundredLines,
            M::FileSmallFunctionRatio,
        ],
        K::UnusedFunction => &[M::FunctionReferences],
        K::MissingDocumentationSet => &[M::DocumentationMissingRequiredDocs],
        K::MissingUserGuide => &[M::DocumentationMissingUserTopics],
        K::MissingReportSchemaDocs | K::MissingMetricsModelDocs | K::MissingArchitectureDocs => {
            &[M::DocumentationRisk]
        }
        K::StaleCliDocumentation => &[M::DocumentationMissingCliFlags],
        K::StaleSchemaDocumentation => &[M::DocumentationMissingSchemaFields],
        K::DependencyCycle => &[
            M::DependencyCycleFiles,
            M::DependencyCycleEdges,
            M::DependencyCycleDensityPercent,
        ],
        K::DependencyHub => &[
            M::DependencyDepth,
            M::DependencyInstabilityPercent,
            M::DependencyFanOut,
            M::DependencyFanIn,
            M::DependencyTransitiveFanOut,
            M::DependencyTransitiveFanIn,
        ],
        K::UnityAssemblyCycle
        | K::UnityAssemblyHub
        | K::UnityUnresolvedAssemblyReference
        | K::UnityRuntimeEditorDependency
        | K::UnityDuplicateGuid
        | K::UnityMissingMeta
        | K::UnityOrphanMeta
        | K::UnityBrokenAssetReference
        | K::UnityMissingScript
        | K::UnityNonTextSerialization
        | K::UnitySceneBuildDrift
        | K::UnityLargeScene
        | K::UnityLargePrefab
        | K::UnitySerializedFieldBloat
        | K::UnityLifecycleOverload
        | K::UnityExpensiveFrameCall
        | K::UnityEditorApiInRuntime
        | K::UnityUnbalancedEventSubscription => &[M::GroupSize],
        K::SimilarFunctions
        | K::RepeatedLiteral
        | K::RepeatedErrorPattern
        | K::TestDuplication
        | K::HappyPathOnlyTests
        | K::FileNamingDrift
        | K::DirectoryDrift
        | K::DataClump
        | K::ParallelImplementation
        | K::ShadowedAbstraction
        | K::DuplicateTypeShape
        | K::ConfigKeyDrift
        | K::FixtureFactoryDrift
        | K::GenericBucketDrift
        | K::AdapterBoundaryBypass
        | K::StaleCompatibilityPath => &[M::GroupSize],
    }
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
        K::UnityAssemblyCycle
        | K::UnityAssemblyHub
        | K::UnityUnresolvedAssemblyReference
        | K::UnityRuntimeEditorDependency
        | K::UnityBrokenAssetReference
        | K::UnityMissingScript
        | K::UnityExpensiveFrameCall
        | K::UnityEditorApiInRuntime => A::ReduceDependencyCoupling,
        K::UnityLargeScene
        | K::UnityLargePrefab
        | K::UnitySerializedFieldBloat
        | K::UnityLifecycleOverload => A::DecomposeResponsibility,
        K::UnityDuplicateGuid
        | K::UnityMissingMeta
        | K::UnityOrphanMeta
        | K::UnityNonTextSerialization
        | K::UnitySceneBuildDrift => A::StandardizeNaming,
        K::UnityUnbalancedEventSubscription => A::StrengthenTestCoverage,
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

pub(crate) fn entity_scope(kind: FindingKind) -> EntityScope {
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
        K::UnityAssemblyCycle => E::FindingGroup,
        K::UnitySerializedFieldBloat
        | K::UnityLifecycleOverload
        | K::UnityUnbalancedEventSubscription => E::Type,
        K::UnityAssemblyHub
        | K::UnityUnresolvedAssemblyReference
        | K::UnityRuntimeEditorDependency
        | K::UnityDuplicateGuid
        | K::UnityMissingMeta
        | K::UnityOrphanMeta
        | K::UnityBrokenAssetReference
        | K::UnityMissingScript
        | K::UnityNonTextSerialization
        | K::UnitySceneBuildDrift
        | K::UnityLargeScene
        | K::UnityLargePrefab
        | K::UnityExpensiveFrameCall
        | K::UnityEditorApiInRuntime => E::File,
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
        K::DependencyCycle
        | K::DependencyHub
        | K::UnityAssemblyCycle
        | K::UnityAssemblyHub
        | K::UnityUnresolvedAssemblyReference
        | K::UnityRuntimeEditorDependency
        | K::UnityBrokenAssetReference
        | K::UnityMissingScript => A::GraphAnalysis,
        K::UnityLargeScene
        | K::UnityLargePrefab
        | K::UnitySerializedFieldBloat
        | K::UnityLifecycleOverload => A::Threshold,
        K::UnityDuplicateGuid
        | K::UnityMissingMeta
        | K::UnityOrphanMeta
        | K::UnityNonTextSerialization
        | K::UnitySceneBuildDrift => A::RepositoryAudit,
        K::UnityExpensiveFrameCall
        | K::UnityEditorApiInRuntime
        | K::UnityUnbalancedEventSubscription => A::Heuristic,
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
            | K::UnityAssemblyCycle
            | K::UnityRuntimeEditorDependency
            | K::UnityDuplicateGuid
            | K::UnityMissingMeta
            | K::UnityOrphanMeta
            | K::UnityBrokenAssetReference
            | K::UnityMissingScript
            | K::UnityNonTextSerialization
            | K::UnityLargeScene
            | K::UnityLargePrefab
            | K::UnitySerializedFieldBloat
            | K::UnityLifecycleOverload
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
    const UNUSED: &[&str] = &[
        "rust",
        "javascript",
        "typescript",
        "tsx",
        "python",
        "go",
        "csharp",
    ];
    const GRAPH: &[&str] = &[
        "rust",
        "javascript",
        "typescript",
        "tsx",
        "python",
        "ruby",
        "c",
        "cpp",
        "csharp",
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
        K::UnityAssemblyCycle
        | K::UnityAssemblyHub
        | K::UnityUnresolvedAssemblyReference
        | K::UnityRuntimeEditorDependency
        | K::UnityDuplicateGuid
        | K::UnityMissingMeta
        | K::UnityOrphanMeta
        | K::UnityBrokenAssetReference
        | K::UnityMissingScript
        | K::UnityNonTextSerialization
        | K::UnitySceneBuildDrift
        | K::UnityLargeScene
        | K::UnityLargePrefab
        | K::UnitySerializedFieldBloat
        | K::UnityLifecycleOverload
        | K::UnityExpensiveFrameCall
        | K::UnityEditorApiInRuntime
        | K::UnityUnbalancedEventSubscription => &["unity"],
        _ => ALL_PARSED,
    }
}

pub(crate) fn default_detection_reliability(kind: FindingKind) -> f64 {
    use FindingKind as K;

    match kind {
        K::SimilarFunctions
        | K::RepeatedErrorPattern
        | K::TestDuplication
        | K::DataClump
        | K::ConfigKeyDrift
        | K::FixtureFactoryDrift => 0.85,
        K::RepeatedLiteral => 0.75,
        K::DuplicateTypeShape => 0.80,
        K::AdapterBoundaryBypass => 0.65,
        K::GenericBucketDrift
        | K::StaleCompatibilityPath
        | K::HappyPathOnlyTests
        | K::FileNamingDrift
        | K::DirectoryDrift
        | K::FunctionProliferation
        | K::UnusedFunction
        | K::ParallelImplementation
        | K::ShadowedAbstraction => 0.60,
        K::MissingDocumentationSet
        | K::MissingUserGuide
        | K::MissingReportSchemaDocs
        | K::MissingMetricsModelDocs
        | K::MissingArchitectureDocs
        | K::StaleCliDocumentation
        | K::StaleSchemaDocumentation => 0.95,
        K::DependencyCycle
        | K::DependencyHub
        | K::ReadabilityRisk
        | K::UnityAssemblyCycle
        | K::UnityRuntimeEditorDependency
        | K::UnityDuplicateGuid
        | K::UnityMissingMeta
        | K::UnityOrphanMeta
        | K::UnityBrokenAssetReference
        | K::UnityMissingScript
        | K::UnityNonTextSerialization
        | K::UnityLargeScene
        | K::UnityLargePrefab
        | K::UnitySerializedFieldBloat
        | K::UnityLifecycleOverload
        | K::UnityEditorApiInRuntime => 0.90,
        K::UnityAssemblyHub
        | K::UnityUnresolvedAssemblyReference
        | K::UnitySceneBuildDrift
        | K::UnityExpensiveFrameCall
        | K::UnityUnbalancedEventSubscription => 0.65,
        K::DebtMarker
        | K::LargeFile
        | K::LargeDirectory
        | K::LongFunction
        | K::ComplexFunction
        | K::DeepNesting
        | K::ManyParameters
        | K::LargeType
        | K::LargePublicSurface
        | K::ImportHeavyFile => 1.0,
    }
}

pub(crate) fn default_interpretation_reliability(kind: FindingKind) -> f64 {
    match kind {
        FindingKind::UnityUnbalancedEventSubscription => 0.55,
        FindingKind::UnityExpensiveFrameCall => 0.70,
        FindingKind::UnityAssemblyHub | FindingKind::UnitySceneBuildDrift => 0.80,
        FindingKind::UnityUnresolvedAssemblyReference => 0.85,
        _ => 0.90,
    }
}

pub(crate) fn impact(kind: FindingKind) -> f64 {
    use FindingKind as K;

    match kind {
        K::DebtMarker => 25.0,
        K::RepeatedLiteral => 30.0,
        K::HappyPathOnlyTests | K::ShadowedAbstraction | K::GenericBucketDrift => 35.0,
        K::FileNamingDrift => 40.0,
        K::TestDuplication
        | K::AdapterBoundaryBypass
        | K::StaleCompatibilityPath
        | K::ParallelImplementation => 45.0,
        K::ConfigKeyDrift | K::FixtureFactoryDrift => 50.0,
        K::LargePublicSurface
        | K::ImportHeavyFile
        | K::FunctionProliferation
        | K::UnusedFunction => 60.0,
        K::LargeFile
        | K::LargeDirectory
        | K::RepeatedErrorPattern
        | K::DirectoryDrift
        | K::DataClump
        | K::DuplicateTypeShape => 65.0,
        K::MissingDocumentationSet
        | K::MissingUserGuide
        | K::MissingMetricsModelDocs
        | K::MissingArchitectureDocs
        | K::LongFunction
        | K::DeepNesting
        | K::ManyParameters
        | K::LargeType => 70.0,
        K::ReadabilityRisk | K::StaleCliDocumentation | K::DependencyHub => 75.0,
        K::SimilarFunctions => 80.0,
        K::DependencyCycle
        | K::UnityAssemblyCycle
        | K::UnityRuntimeEditorDependency
        | K::UnityDuplicateGuid
        | K::UnityMissingScript => 85.0,
        K::UnityAssemblyHub
        | K::UnityUnresolvedAssemblyReference
        | K::UnityMissingMeta
        | K::UnityOrphanMeta
        | K::UnityBrokenAssetReference
        | K::UnityNonTextSerialization
        | K::UnityLargeScene
        | K::UnityLargePrefab
        | K::UnitySerializedFieldBloat
        | K::UnityLifecycleOverload
        | K::UnityExpensiveFrameCall
        | K::UnityEditorApiInRuntime => 65.0,
        K::UnitySceneBuildDrift | K::UnityUnbalancedEventSubscription => 35.0,
        K::ComplexFunction | K::MissingReportSchemaDocs | K::StaleSchemaDocumentation => 90.0,
    }
}

pub(crate) fn actionability(kind: FindingKind) -> f64 {
    use FindingKind as K;

    match kind {
        K::GenericBucketDrift => 35.0,
        K::RepeatedLiteral | K::HappyPathOnlyTests => 40.0,
        K::ShadowedAbstraction | K::StaleCompatibilityPath | K::AdapterBoundaryBypass => 45.0,
        K::ParallelImplementation => 50.0,
        K::TestDuplication => 55.0,
        K::DebtMarker | K::FileNamingDrift | K::DirectoryDrift => 60.0,
        K::ConfigKeyDrift | K::FixtureFactoryDrift => 65.0,
        K::MissingMetricsModelDocs | K::MissingArchitectureDocs | K::RepeatedErrorPattern => 70.0,
        K::LargeDirectory
        | K::ImportHeavyFile
        | K::LargePublicSurface
        | K::FunctionProliferation
        | K::UnusedFunction
        | K::DataClump
        | K::DependencyHub => 75.0,
        K::ReadabilityRisk => 80.0,
        K::MissingDocumentationSet
        | K::MissingUserGuide
        | K::MissingReportSchemaDocs
        | K::StaleCliDocumentation
        | K::StaleSchemaDocumentation
        | K::DependencyCycle
        | K::LargeFile
        | K::LongFunction
        | K::ComplexFunction
        | K::DeepNesting
        | K::ManyParameters
        | K::LargeType
        | K::SimilarFunctions
        | K::DuplicateTypeShape
        | K::UnityAssemblyCycle
        | K::UnityAssemblyHub
        | K::UnityUnresolvedAssemblyReference
        | K::UnityRuntimeEditorDependency
        | K::UnityDuplicateGuid
        | K::UnityMissingMeta
        | K::UnityOrphanMeta
        | K::UnityBrokenAssetReference
        | K::UnityMissingScript
        | K::UnityNonTextSerialization
        | K::UnitySceneBuildDrift
        | K::UnityLargeScene
        | K::UnityLargePrefab
        | K::UnitySerializedFieldBloat
        | K::UnityLifecycleOverload
        | K::UnityExpensiveFrameCall
        | K::UnityEditorApiInRuntime
        | K::UnityUnbalancedEventSubscription => 85.0,
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
    fn atomic_readability_signals_share_issue_contract() {
        let manifest = detector_manifest();
        let long_function = manifest
            .iter()
            .find(|entry| entry.kind == FindingKind::LongFunction)
            .unwrap();
        assert_eq!(long_function.mechanism, SignalMechanism::CognitiveLoad);
        assert_eq!(long_function.action, RefactorAction::SimplifyFunction);
        assert_eq!(long_function.entity_scope, EntityScope::Function);
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
            entry.name == MetricId::FileIsTest
                && entry.scale == MetricScale::Boolean
                && entry.direction == MetricDirection::ContextOnly
        }));
        assert!(manifest.iter().any(|entry| {
            entry.name == MetricId::DirectorySourceFiles
                && entry.entity_scope == EntityScope::Directory
                && entry.direction == MetricDirection::HigherIsMorePressure
        }));
    }

    #[test]
    fn detector_specs_have_unique_typed_inputs_and_explicit_ranking_policy() {
        for entry in detector_manifest() {
            let mut inputs = entry.input_metrics.clone();
            inputs.sort_unstable();
            inputs.dedup();

            assert_eq!(inputs.len(), entry.input_metrics.len(), "{:?}", entry.kind);
            assert!((0.0..=1.0).contains(&entry.default_detection_reliability));
            assert!((0.0..=1.0).contains(&entry.default_interpretation_reliability));
            assert!((0.0..=100.0).contains(&entry.impact));
            assert!((0.0..=100.0).contains(&entry.actionability));
        }
    }
}
