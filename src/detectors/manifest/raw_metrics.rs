use crate::model::{EntityScope, MetricDirection, MetricScale, RawMetricManifestEntry};

struct RawMetricSpec {
    name: &'static str,
    entity_scope: EntityScope,
    unit: &'static str,
    scale: MetricScale,
    direction: MetricDirection,
    description: &'static str,
}

impl RawMetricSpec {
    fn to_manifest_entry(&self) -> RawMetricManifestEntry {
        RawMetricManifestEntry {
            name: self.name.to_string(),
            entity_scope: self.entity_scope,
            unit: self.unit.to_string(),
            scale: self.scale,
            direction: self.direction,
            description: self.description.to_string(),
        }
    }
}

const fn pressure_count(
    name: &'static str,
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
    name: &'static str,
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
    name: &'static str,
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
        "file.loc",
        EntityScope::File,
        "lines",
        "physical source lines in the file",
    ),
    pressure_count(
        "file.imports",
        EntityScope::File,
        "imports",
        "top-level import or use declarations",
    ),
    pressure_count(
        "file.public_items",
        EntityScope::File,
        "items",
        "public or exported top-level items",
    ),
    pressure_count(
        "file.directory_source_files",
        EntityScope::Directory,
        "files",
        "direct source files in the parent directory",
    ),
    context_boolean(
        "file.is_test",
        EntityScope::File,
        "whether the path is classified as test source",
    ),
    pressure_count(
        "function.loc",
        EntityScope::Function,
        "lines",
        "physical line span of the function",
    ),
    pressure_count(
        "function.complexity",
        EntityScope::Function,
        "paths",
        "estimated cyclomatic complexity",
    ),
    pressure_count(
        "function.nesting_depth",
        EntityScope::Function,
        "levels",
        "maximum nested control-flow depth",
    ),
    pressure_count(
        "function.parameter_count",
        EntityScope::Function,
        "parameters",
        "declared function parameters",
    ),
    context_boolean(
        "function.is_test",
        EntityScope::Function,
        "whether the function belongs to test source",
    ),
    pressure_count(
        "type.loc",
        EntityScope::Type,
        "lines",
        "physical line span of the type",
    ),
    pressure_count(
        "type.member_count",
        EntityScope::Type,
        "members",
        "fields, variants, methods, signatures, or equivalent members",
    ),
    context_boolean(
        "type.is_test",
        EntityScope::Type,
        "whether the type belongs to test source",
    ),
    pressure_count(
        "churn.commits_touched",
        EntityScope::File,
        "commits",
        "non-merge commits touching the file in the configured window",
    ),
    context_count(
        "churn.lines_added",
        EntityScope::File,
        "lines",
        "lines added in included commits",
    ),
    context_count(
        "churn.lines_deleted",
        EntityScope::File,
        "lines",
        "lines deleted in included commits",
    ),
    pressure_count(
        "churn.authors_count",
        EntityScope::File,
        "authors",
        "distinct authors touching the file",
    ),
    pressure_count(
        "churn.recent_weighted_churn",
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
