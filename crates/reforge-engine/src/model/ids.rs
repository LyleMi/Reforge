use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(usize)]
pub enum MetricId {
    #[serde(rename = "file.loc")]
    FileLoc,
    #[serde(rename = "file.imports")]
    FileImports,
    #[serde(rename = "file.public_items")]
    FilePublicItems,
    #[serde(rename = "file.is_test")]
    FileIsTest,
    #[serde(rename = "directory.source_files")]
    DirectorySourceFiles,
    #[serde(rename = "function.loc")]
    FunctionLoc,
    #[serde(rename = "function.complexity")]
    FunctionComplexity,
    #[serde(rename = "function.nesting_depth")]
    FunctionNestingDepth,
    #[serde(rename = "function.parameter_count")]
    FunctionParameterCount,
    #[serde(rename = "function.is_test")]
    FunctionIsTest,
    #[serde(rename = "type.loc")]
    TypeLoc,
    #[serde(rename = "type.member_count")]
    TypeMemberCount,
    #[serde(rename = "type.is_test")]
    TypeIsTest,
    #[serde(rename = "churn.commits_touched")]
    ChurnCommitsTouched,
    #[serde(rename = "churn.lines_added")]
    ChurnLinesAdded,
    #[serde(rename = "churn.lines_deleted")]
    ChurnLinesDeleted,
    #[serde(rename = "churn.authors_count")]
    ChurnAuthorsCount,
    #[serde(rename = "churn.recent_weighted_churn")]
    ChurnRecentWeighted,
    #[serde(rename = "group.size")]
    GroupSize,
    #[serde(rename = "file.function_count")]
    FileFunctionCount,
    #[serde(rename = "file.functions_per_100_lines")]
    FileFunctionsPerHundredLines,
    #[serde(rename = "file.small_function_ratio")]
    FileSmallFunctionRatio,
    #[serde(rename = "dependency.cycle_files")]
    DependencyCycleFiles,
    #[serde(rename = "dependency.cycle_edges")]
    DependencyCycleEdges,
    #[serde(rename = "dependency.cycle_density_percent")]
    DependencyCycleDensityPercent,
    #[serde(rename = "dependency.depth")]
    DependencyDepth,
    #[serde(rename = "dependency.instability_percent")]
    DependencyInstabilityPercent,
    #[serde(rename = "dependency.fan_out")]
    DependencyFanOut,
    #[serde(rename = "dependency.fan_in")]
    DependencyFanIn,
    #[serde(rename = "dependency.transitive_fan_out")]
    DependencyTransitiveFanOut,
    #[serde(rename = "dependency.transitive_fan_in")]
    DependencyTransitiveFanIn,
    #[serde(rename = "function.references")]
    FunctionReferences,
    #[serde(rename = "documentation.missing_user_topics")]
    DocumentationMissingUserTopics,
    #[serde(rename = "documentation.risk")]
    DocumentationRisk,
    #[serde(rename = "documentation.missing_cli_flags")]
    DocumentationMissingCliFlags,
    #[serde(rename = "documentation.missing_schema_fields")]
    DocumentationMissingSchemaFields,
    #[serde(rename = "flow.module_hops")]
    FlowModuleHops,
    #[serde(rename = "flow.call_edges")]
    FlowCallEdges,
    #[serde(rename = "flow.path_steps")]
    FlowPathSteps,
    #[serde(rename = "flow.unresolved_edges")]
    FlowUnresolvedEdges,
    #[serde(rename = "flow.policy_conforming_paths")]
    FlowPolicyConformingPaths,
    #[serde(rename = "flow.policy_bypass_paths")]
    FlowPolicyBypassPaths,
    #[serde(rename = "flow.function_hops")]
    FlowFunctionHops,
    #[serde(rename = "flow.relay_ratio_percent")]
    FlowRelayRatioPercent,
    #[serde(rename = "flow.sink_count")]
    FlowSinkCount,
    #[serde(rename = "flow.branch_count")]
    FlowBranchCount,
    #[serde(rename = "flow.module_count")]
    FlowModuleCount,
    #[serde(rename = "flow.max_path_steps")]
    FlowMaxPathSteps,
}

impl MetricId {
    pub const fn as_str(self) -> &'static str {
        METRIC_IDS[self as usize]
    }
}

const METRIC_IDS: [&str; 48] = [
    "file.loc",
    "file.imports",
    "file.public_items",
    "file.is_test",
    "directory.source_files",
    "function.loc",
    "function.complexity",
    "function.nesting_depth",
    "function.parameter_count",
    "function.is_test",
    "type.loc",
    "type.member_count",
    "type.is_test",
    "churn.commits_touched",
    "churn.lines_added",
    "churn.lines_deleted",
    "churn.authors_count",
    "churn.recent_weighted_churn",
    "group.size",
    "file.function_count",
    "file.functions_per_100_lines",
    "file.small_function_ratio",
    "dependency.cycle_files",
    "dependency.cycle_edges",
    "dependency.cycle_density_percent",
    "dependency.depth",
    "dependency.instability_percent",
    "dependency.fan_out",
    "dependency.fan_in",
    "dependency.transitive_fan_out",
    "dependency.transitive_fan_in",
    "function.references",
    "documentation.missing_user_topics",
    "documentation.risk",
    "documentation.missing_cli_flags",
    "documentation.missing_schema_fields",
    "flow.module_hops",
    "flow.call_edges",
    "flow.path_steps",
    "flow.unresolved_edges",
    "flow.policy_conforming_paths",
    "flow.policy_bypass_paths",
    "flow.function_hops",
    "flow.relay_ratio_percent",
    "flow.sink_count",
    "flow.branch_count",
    "flow.module_count",
    "flow.max_path_steps",
];

impl std::fmt::Display for MetricId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}
