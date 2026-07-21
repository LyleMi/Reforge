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

include!("manifest/policy.rs");
