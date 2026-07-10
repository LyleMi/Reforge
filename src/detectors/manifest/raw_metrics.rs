use crate::model::{EntityScope, MetricDirection, MetricId, MetricScale, RawMetricManifestEntry};

struct RawMetricSpec {
    name: MetricId,
    entity_scope: EntityScope,
    unit: &'static str,
    scale: MetricScale,
    direction: MetricDirection,
    description: &'static str,
}

impl RawMetricSpec {
    fn to_manifest_entry(&self) -> RawMetricManifestEntry {
        RawMetricManifestEntry {
            name: self.name,
            entity_scope: self.entity_scope,
            unit: self.unit.to_string(),
            scale: self.scale,
            direction: self.direction,
            description: self.description.to_string(),
        }
    }
}

const fn pressure_count(
    name: MetricId,
    entity_scope: EntityScope,
    unit: &'static str,
    description: &'static str,
) -> RawMetricSpec {
    RawMetricSpec {
        name,
        entity_scope,
        unit,
        scale: MetricScale::Count,
        direction: MetricDirection::HigherIsMorePressure,
        description,
    }
}

const fn context_count(
    name: MetricId,
    entity_scope: EntityScope,
    unit: &'static str,
    description: &'static str,
) -> RawMetricSpec {
    RawMetricSpec {
        name,
        entity_scope,
        unit,
        scale: MetricScale::Count,
        direction: MetricDirection::ContextOnly,
        description,
    }
}

const fn context_boolean(
    name: MetricId,
    entity_scope: EntityScope,
    description: &'static str,
) -> RawMetricSpec {
    RawMetricSpec {
        name,
        entity_scope,
        unit: "boolean",
        scale: MetricScale::Boolean,
        direction: MetricDirection::ContextOnly,
        description,
    }
}

const RAW_METRIC_SPECS: &[RawMetricSpec] = &[
    pressure_count(
        MetricId::FileLoc,
        EntityScope::File,
        "lines",
        "physical source lines in the file",
    ),
    pressure_count(
        MetricId::FileImports,
        EntityScope::File,
        "imports",
        "top-level import or use declarations",
    ),
    pressure_count(
        MetricId::FilePublicItems,
        EntityScope::File,
        "items",
        "public or exported top-level items",
    ),
    pressure_count(
        MetricId::DirectorySourceFiles,
        EntityScope::Directory,
        "files",
        "direct source files in the parent directory",
    ),
    context_boolean(
        MetricId::FileIsTest,
        EntityScope::File,
        "whether the path is classified as test source",
    ),
    pressure_count(
        MetricId::FunctionLoc,
        EntityScope::Function,
        "lines",
        "physical line span of the function",
    ),
    pressure_count(
        MetricId::FunctionComplexity,
        EntityScope::Function,
        "paths",
        "estimated cyclomatic complexity",
    ),
    pressure_count(
        MetricId::FunctionNestingDepth,
        EntityScope::Function,
        "levels",
        "maximum nested control-flow depth",
    ),
    pressure_count(
        MetricId::FunctionParameterCount,
        EntityScope::Function,
        "parameters",
        "declared function parameters",
    ),
    context_boolean(
        MetricId::FunctionIsTest,
        EntityScope::Function,
        "whether the function belongs to test source",
    ),
    pressure_count(
        MetricId::TypeLoc,
        EntityScope::Type,
        "lines",
        "physical line span of the type",
    ),
    pressure_count(
        MetricId::TypeMemberCount,
        EntityScope::Type,
        "members",
        "fields, variants, methods, signatures, or equivalent members",
    ),
    context_boolean(
        MetricId::TypeIsTest,
        EntityScope::Type,
        "whether the type belongs to test source",
    ),
    pressure_count(
        MetricId::ChurnCommitsTouched,
        EntityScope::File,
        "commits",
        "non-merge commits touching the file in the configured window",
    ),
    context_count(
        MetricId::ChurnLinesAdded,
        EntityScope::File,
        "lines",
        "lines added in included commits",
    ),
    context_count(
        MetricId::ChurnLinesDeleted,
        EntityScope::File,
        "lines",
        "lines deleted in included commits",
    ),
    pressure_count(
        MetricId::ChurnAuthorsCount,
        EntityScope::File,
        "authors",
        "distinct authors touching the file",
    ),
    pressure_count(
        MetricId::ChurnRecentWeighted,
        EntityScope::File,
        "weighted_lines",
        "time-decayed added and deleted lines",
    ),
];

pub(crate) fn raw_metric_manifest() -> Vec<RawMetricManifestEntry> {
    RAW_METRIC_SPECS
        .iter()
        .map(RawMetricSpec::to_manifest_entry)
        .collect()
}
