pub(crate) fn input_metrics(kind: FindingKind) -> &'static [MetricId] {
    use FindingKind as K;
    use MetricId as M;

    if is_group_size_finding(kind) {
        return &[M::GroupSize];
    }
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
        K::AdapterFlowBypass => &[
            M::FlowModuleHops,
            M::FlowCallEdges,
            M::FlowPathSteps,
            M::FlowUnresolvedEdges,
            M::FlowPolicyConformingPaths,
            M::FlowPolicyBypassPaths,
        ],
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
        _ => unreachable!("group-size findings returned before metric dispatch"),
    }
}

fn is_group_size_finding(kind: FindingKind) -> bool {
    use FindingKind as K;
    matches!(
        kind,
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
            | K::UnityUnbalancedEventSubscription
            | K::SimilarFunctions
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
            | K::StaleCompatibilityPath
    )
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
        K::AdapterFlowBypass => A::ReduceDependencyCoupling,
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

    if matches!(
        kind,
        K::MissingDocumentationSet
            | K::MissingUserGuide
            | K::MissingReportSchemaDocs
            | K::MissingMetricsModelDocs
            | K::MissingArchitectureDocs
            | K::StaleCliDocumentation
            | K::StaleSchemaDocumentation
    ) {
        return E::Repository;
    }
    if matches!(
        kind,
        K::LargeDirectory | K::FileNamingDrift | K::DirectoryDrift | K::GenericBucketDrift
    ) {
        return E::Directory;
    }
    if matches!(
        kind,
        K::LongFunction
            | K::ComplexFunction
            | K::DeepNesting
            | K::ManyParameters
            | K::ReadabilityRisk
    ) {
        return E::Function;
    }
    if matches!(
        kind,
        K::LargeType
            | K::UnitySerializedFieldBloat
            | K::UnityLifecycleOverload
            | K::UnityUnbalancedEventSubscription
    ) {
        return E::Type;
    }
    if kind == K::AdapterFlowBypass {
        return E::FindingGroup;
    }
    if is_group_scoped_finding(kind) {
        return E::FindingGroup;
    }
    E::File
}

fn is_group_scoped_finding(kind: FindingKind) -> bool {
    use FindingKind as K;
    matches!(
        kind,
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
            | K::DependencyCycle
            | K::UnityAssemblyCycle
    )
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
        K::AdapterFlowBypass => A::GraphAnalysis,
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
            | K::AdapterFlowBypass
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
        "bash",
        "powershell",
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
        K::AdapterFlowBypass => &["rust"],
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
    fn detector_specs_have_unique_typed_inputs() {
        for entry in detector_manifest() {
            let mut inputs = entry.input_metrics.clone();
            inputs.sort_unstable();
            inputs.dedup();

            assert_eq!(inputs.len(), entry.input_metrics.len(), "{:?}", entry.kind);
        }
    }

    #[test]
    fn script_languages_are_parsed_detector_scope_only() {
        let manifest = detector_manifest();
        let long_function = manifest
            .iter()
            .find(|entry| entry.kind == FindingKind::LongFunction)
            .unwrap();
        assert!(long_function.supported_languages.contains(&"bash".to_string()));
        assert!(
            long_function
                .supported_languages
                .contains(&"powershell".to_string())
        );

        for kind in [
            FindingKind::UnusedFunction,
            FindingKind::DependencyCycle,
            FindingKind::DependencyHub,
        ] {
            let entry = manifest.iter().find(|entry| entry.kind == kind).unwrap();
            assert!(!entry.supported_languages.contains(&"bash".to_string()));
            assert!(!entry.supported_languages.contains(&"powershell".to_string()));
        }
    }
}
