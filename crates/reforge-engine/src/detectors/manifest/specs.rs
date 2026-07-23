use ObservationSource as O;
use SubjectKind as S;
use IssueFamily as F;
use MetricId as M;
use Rule as K;

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
const FLOW: &[&str] = &["rust", "javascript", "typescript", "tsx", "python"];
const RUST: &[&str] = &["rust"];
const REPOSITORY: &[&str] = &["repository"];
const PATHS: &[&str] = &["language_neutral_paths"];

const fn seed(
    kind: Rule,
    analysis: &'static str,
    issue: (IssueFamily, SubjectKind),
    detector: (&'static [&'static str], &'static [MetricId]),
) -> RuleSpecSeed {
    RuleSpecSeed {
        kind,
        analysis,
        family: issue.0,
        subject: issue.1,
        observation_source: seed_observation_source(kind, issue.1),
        languages: detector.0,
        measurements: detector.1,
        description: rule_description(kind),
    }
}

const fn seed_observation_source(kind: Rule, subject: SubjectKind) -> ObservationSource {
    match kind {
        K::SimilarFunctions => O::FunctionPairs,
        K::DataClump | K::ParallelImplementation | K::ShadowedAbstraction => O::Functions,
        K::DuplicateTypeShape => O::Types,
        K::DependencyCycle | K::DependencyHub => O::DependencyNodes,
        K::AdapterFlowBypass | K::ExcessiveRelay | K::FlowFanOut => O::DataflowSources,
        _ => match subject {
            S::Repository => O::Repositories,
            S::Directory => O::Directories,
            S::File => O::Files,
            S::Symbol => O::Functions,
            S::Group => O::Files,
        },
    }
}

const fn rule_description(kind: Rule) -> &'static str {
    match kind {
        K::LargeFile
        | K::LargeDirectory
        | K::DebtMarker
        | K::SimilarFunctions
        | K::LongFunction
        | K::ComplexFunction
        | K::DeepNesting
        | K::ManyParameters
        | K::LargeType
        | K::LargePublicSurface
        | K::ImportHeavyFile
        | K::FunctionProliferation
        | K::UnusedFunction => codebase_metric_description(kind),
        K::RepeatedLiteral
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
        | K::AdapterBoundaryBypass => codebase_pattern_description(kind),
        K::AdapterFlowBypass | K::ExcessiveRelay | K::FlowFanOut => {
            dataflow_rule_description(kind)
        }
        K::StaleCompatibilityPath
        | K::MissingUserGuide
        | K::MissingReportSchemaDocs
        | K::MissingMetricsModelDocs
        | K::MissingArchitectureDocs
        | K::StaleCliDocumentation
        | K::StaleSchemaDocumentation
        | K::DependencyCycle
        | K::DependencyHub => codebase_repository_description(kind),
    }
}

const fn codebase_metric_description(kind: Rule) -> &'static str {
    match kind {
        K::LargeFile => "Reports source files whose line count exceeds the configured limit.",
        K::LargeDirectory => {
            "Reports directories whose direct source-file count exceeds the configured limit."
        }
        K::DebtMarker => "Reports TODO and FIXME comments that declare unresolved work.",
        K::SimilarFunctions => {
            "Groups functions with sufficiently similar normalized implementation bodies."
        }
        K::LongFunction => "Reports functions whose line span exceeds the configured limit.",
        K::ComplexFunction => {
            "Reports functions whose estimated cyclomatic complexity exceeds the configured limit."
        }
        K::DeepNesting => {
            "Reports functions whose nested control-flow depth exceeds the configured limit."
        }
        K::ManyParameters => {
            "Reports functions whose parameter count exceeds the configured limit."
        }
        K::LargeType => {
            "Reports types whose line span or member count exceeds configured limits."
        }
        K::LargePublicSurface => {
            "Reports files that expose more public items than the configured limit."
        }
        K::ImportHeavyFile => {
            "Reports files whose import count exceeds the configured limit."
        }
        K::FunctionProliferation => {
            "Reports files combining high function count, density, and small-function ratio."
        }
        K::UnusedFunction => {
            "Reports private functions with no project-wide reference outside their own body."
        }
        _ => "",
    }
}

const fn codebase_pattern_description(kind: Rule) -> &'static str {
    match kind {
        K::RepeatedLiteral => {
            "Groups repeated string or numeric literals that may need one owner."
        }
        K::RepeatedErrorPattern => {
            "Groups repeated catch, except, or error-handling implementations."
        }
        K::TestDuplication => "Groups repeated setup, fixture, mock, fake, or before-hook patterns.",
        K::HappyPathOnlyTests => {
            "Reports test groups with assertion evidence but no failure or boundary cases."
        }
        K::FileNamingDrift => {
            "Reports directories that mix incompatible source-file naming conventions."
        }
        K::DirectoryDrift => {
            "Reports directories whose files suggest too many unrelated concepts."
        }
        K::DataClump => "Groups parameter combinations repeated across multiple functions.",
        K::ParallelImplementation => {
            "Groups similarly named capabilities implemented independently across files."
        }
        K::ShadowedAbstraction => {
            "Groups local helpers that duplicate an existing shared abstraction."
        }
        K::DuplicateTypeShape => {
            "Groups type declarations with substantially overlapping field shapes."
        }
        K::ConfigKeyDrift => {
            "Groups repeated configuration, route, environment, endpoint, or token keys."
        }
        K::FixtureFactoryDrift => {
            "Groups repeated fixture, factory, mock, fake, or sample concepts in tests."
        }
        K::GenericBucketDrift => {
            "Reports generic shared or utility directories that accumulate unrelated concepts."
        }
        K::AdapterBoundaryBypass => {
            "Reports heuristic direct access around a named adapter or boundary module."
        }
        _ => "",
    }
}

const fn dataflow_rule_description(kind: Rule) -> &'static str {
    match kind {
        K::AdapterFlowBypass => {
            "Reports exact policy-protected value paths that bypass their declared adapter."
        }
        K::ExcessiveRelay => {
            "Reports exact value paths dominated by cross-function and cross-module forwarding."
        }
        K::FlowFanOut => {
            "Reports source values that reach many distinct sinks across multiple modules."
        }
        _ => "",
    }
}

const fn codebase_repository_description(kind: Rule) -> &'static str {
    match kind {
        K::StaleCompatibilityPath => {
            "Reports compatibility paths that lack an explicit owner or retirement plan."
        }
        K::MissingUserGuide => "Reports missing installation, usage, output, or troubleshooting documentation.",
        K::MissingReportSchemaDocs => {
            "Reports a missing public report-field and compatibility reference."
        }
        K::MissingMetricsModelDocs => {
            "Reports missing documentation for measurements, Evidence, Issues, or Coverage."
        }
        K::MissingArchitectureDocs => {
            "Reports missing documentation for analyzer execution and extension boundaries."
        }
        K::StaleCliDocumentation => {
            "Reports documented CLI surfaces that omit current commands or flags."
        }
        K::StaleSchemaDocumentation => {
            "Reports schema documentation that omits current report fields."
        }
        K::DependencyCycle => {
            "Reports cycles in the resolved project-local source dependency graph."
        }
        K::DependencyHub => {
            "Reports source files with unusually broad or deep resolved dependencies."
        }
        _ => "",
    }
}

const RULE_SPEC_SEEDS: &[RuleSpecSeed] = &[
    seed(K::LargeFile, ANALYSIS_CODEBASE, (F::ResponsibilityDecomposition, S::File), (PATHS, &[M::FileLoc])),
    seed(K::LargeDirectory, ANALYSIS_CODEBASE, (F::DirectoryOrganization, S::Directory), (PATHS, &[M::DirectorySourceFiles])),
    seed(K::DebtMarker, ANALYSIS_CODEBASE, (F::DeclaredDebt, S::File), (PATHS, &[])),
    seed(K::SimilarFunctions, ANALYSIS_CODEBASE, (F::ImplementationDuplication, S::Group), (ALL_PARSED, &[M::GroupSize])),
    seed(K::LongFunction, ANALYSIS_CODEBASE, (F::FunctionReadability, S::Symbol), (ALL_PARSED, &[M::FunctionLoc])),
    seed(K::ComplexFunction, ANALYSIS_CODEBASE, (F::FunctionReadability, S::Symbol), (ALL_PARSED, &[M::FunctionComplexity])),
    seed(K::DeepNesting, ANALYSIS_CODEBASE, (F::FunctionReadability, S::Symbol), (ALL_PARSED, &[M::FunctionNestingDepth])),
    seed(K::ManyParameters, ANALYSIS_CODEBASE, (F::FunctionReadability, S::Symbol), (ALL_PARSED, &[M::FunctionParameterCount])),
    seed(K::LargeType, ANALYSIS_CODEBASE, (F::ResponsibilityDecomposition, S::Symbol), (ALL_PARSED, &[M::TypeLoc, M::TypeMemberCount])),
    seed(K::LargePublicSurface, ANALYSIS_CODEBASE, (F::ModuleSurface, S::File), (ALL_PARSED, &[M::FilePublicItems])),
    seed(K::ImportHeavyFile, ANALYSIS_CODEBASE, (F::ModuleSurface, S::File), (ALL_PARSED, &[M::FileImports])),
    seed(K::FunctionProliferation, ANALYSIS_CODEBASE, (F::ResponsibilityDecomposition, S::File), (ALL_PARSED, &[M::FileFunctionCount, M::FileFunctionsPerHundredLines, M::FileSmallFunctionRatio])),
    seed(K::UnusedFunction, ANALYSIS_CODEBASE, (F::DeadCode, S::File), (UNUSED, &[M::FunctionReferences])),
    seed(K::RepeatedLiteral, ANALYSIS_CODEBASE, (F::LiteralOwnership, S::Group), (ALL_PARSED, &[M::GroupSize])),
    seed(K::RepeatedErrorPattern, ANALYSIS_CODEBASE, (F::ErrorHandlingDuplication, S::Group), (ALL_PARSED, &[M::GroupSize])),
    seed(K::TestDuplication, ANALYSIS_CODEBASE, (F::TestSupport, S::Group), (ALL_PARSED, &[M::GroupSize])),
    seed(K::HappyPathOnlyTests, ANALYSIS_CODEBASE, (F::TestCoverage, S::Group), (ALL_PARSED, &[M::GroupSize])),
    seed(K::FileNamingDrift, ANALYSIS_CODEBASE, (F::Naming, S::Directory), (PATHS, &[M::GroupSize])),
    seed(K::DirectoryDrift, ANALYSIS_CODEBASE, (F::DirectoryOrganization, S::Directory), (ALL_PARSED, &[M::GroupSize])),
    seed(K::DataClump, ANALYSIS_CODEBASE, (F::DataShapeDuplication, S::Group), (ALL_PARSED, &[M::GroupSize])),
    seed(K::ParallelImplementation, ANALYSIS_CODEBASE, (F::ImplementationDuplication, S::Group), (ALL_PARSED, &[M::GroupSize])),
    seed(K::ShadowedAbstraction, ANALYSIS_CODEBASE, (F::ImplementationDuplication, S::Group), (ALL_PARSED, &[M::GroupSize])),
    seed(K::DuplicateTypeShape, ANALYSIS_CODEBASE, (F::DataShapeDuplication, S::Group), (ALL_PARSED, &[M::GroupSize])),
    seed(K::ConfigKeyDrift, ANALYSIS_CODEBASE, (F::LiteralOwnership, S::Group), (ALL_PARSED, &[M::GroupSize])),
    seed(K::FixtureFactoryDrift, ANALYSIS_CODEBASE, (F::TestSupport, S::Group), (ALL_PARSED, &[M::GroupSize])),
    seed(K::GenericBucketDrift, ANALYSIS_CODEBASE, (F::DirectoryOrganization, S::Directory), (ALL_PARSED, &[M::GroupSize])),
    seed(K::AdapterBoundaryBypass, ANALYSIS_CODEBASE, (F::BoundaryIntegrity, S::Group), (ALL_PARSED, &[M::GroupSize])),
    seed(K::AdapterFlowBypass, ANALYSIS_DATAFLOW, (F::BoundaryIntegrity, S::Group), (RUST, &[M::FlowModuleHops, M::FlowCallEdges, M::FlowPathSteps, M::FlowUnresolvedEdges, M::FlowPolicyConformingPaths, M::FlowPolicyBypassPaths])),
    seed(K::ExcessiveRelay, ANALYSIS_DATAFLOW, (F::DataflowOwnership, S::Group), (FLOW, &[M::FlowPathSteps, M::FlowFunctionHops, M::FlowModuleHops, M::FlowRelayRatioPercent])),
    seed(K::FlowFanOut, ANALYSIS_DATAFLOW, (F::DataflowOwnership, S::Group), (FLOW, &[M::FlowSinkCount, M::FlowBranchCount, M::FlowModuleCount, M::FlowMaxPathSteps])),
    seed(K::StaleCompatibilityPath, ANALYSIS_CODEBASE, (F::CompatibilityRetirement, S::Group), (ALL_PARSED, &[M::GroupSize])),
    seed(K::MissingUserGuide, ANALYSIS_CODEBASE, (F::DocumentationIntegrity, S::Repository), (REPOSITORY, &[M::DocumentationMissingUserTopics])),
    seed(K::MissingReportSchemaDocs, ANALYSIS_CODEBASE, (F::DocumentationIntegrity, S::Repository), (REPOSITORY, &[M::DocumentationRisk])),
    seed(K::MissingMetricsModelDocs, ANALYSIS_CODEBASE, (F::DocumentationIntegrity, S::Repository), (REPOSITORY, &[M::DocumentationRisk])),
    seed(K::MissingArchitectureDocs, ANALYSIS_CODEBASE, (F::DocumentationIntegrity, S::Repository), (REPOSITORY, &[M::DocumentationRisk])),
    seed(K::StaleCliDocumentation, ANALYSIS_CODEBASE, (F::DocumentationIntegrity, S::Repository), (REPOSITORY, &[M::DocumentationMissingCliFlags])),
    seed(K::StaleSchemaDocumentation, ANALYSIS_CODEBASE, (F::DocumentationIntegrity, S::Repository), (REPOSITORY, &[M::DocumentationMissingSchemaFields])),
    seed(K::DependencyCycle, ANALYSIS_CODEBASE, (F::DependencyTopology, S::Group), (GRAPH, &[M::DependencyCycleFiles, M::DependencyCycleEdges, M::DependencyCycleDensityPercent])),
    seed(K::DependencyHub, ANALYSIS_CODEBASE, (F::DependencyTopology, S::File), (GRAPH, &[M::DependencyDepth, M::DependencyInstabilityPercent, M::DependencyFanOut, M::DependencyFanIn, M::DependencyTransitiveFanOut, M::DependencyTransitiveFanIn])),
];
