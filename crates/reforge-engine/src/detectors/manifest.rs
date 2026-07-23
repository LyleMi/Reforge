use std::sync::LazyLock;

use crate::model::{
    IssueFamily, MetricId, ObservationSource, Rule, RuleSpec, SubjectKind, serialized_rule,
};

use reforge_schema::{ANALYSIS_CODEBASE, ANALYSIS_DATAFLOW};

#[derive(Clone, Copy)]
struct RuleSpecSeed {
    kind: Rule,
    analysis: &'static str,
    family: IssueFamily,
    subject: SubjectKind,
    observation_source: ObservationSource,
    languages: &'static [&'static str],
    measurements: &'static [MetricId],
    description: &'static str,
}

include!("manifest/specs.rs");

static RULE_SPECS: LazyLock<Vec<RuleSpec>> = LazyLock::new(|| {
    assert_eq!(
        RULE_SPEC_SEEDS.len(),
        Rule::ALL.len(),
        "every executable rule must have one RuleSpec"
    );
    RULE_SPEC_SEEDS
        .iter()
        .copied()
        .map(|seed| RuleSpec {
            kind: seed.kind,
            rule: format!("reforge.{}.{}", seed.analysis, serialized_rule(seed.kind)),
            analysis: seed.analysis.into(),
            family: seed.family,
            subject: seed.subject,
            observation_source: seed.observation_source,
            languages: seed.languages.iter().map(|value| (*value).into()).collect(),
            measurements: seed.measurements.to_vec(),
            description: seed.description.into(),
        })
        .collect()
});

pub(crate) fn rule_registry() -> &'static [RuleSpec] {
    &RULE_SPECS
}

fn rule_spec(rule: Rule) -> &'static RuleSpec {
    rule_registry()
        .iter()
        .find(|entry| entry.kind == rule)
        .expect("every executable rule must have one RuleSpec")
}

pub(crate) fn analysis_name(rule: Rule) -> &'static str {
    rule_spec(rule).analysis.as_str()
}

pub(crate) fn input_metrics(rule: Rule) -> &'static [MetricId] {
    &rule_spec(rule).measurements
}

pub(crate) fn subject_kind(rule: Rule) -> SubjectKind {
    rule_spec(rule).subject
}

pub(crate) fn observation_source(rule: Rule) -> ObservationSource {
    rule_spec(rule).observation_source
}

impl IssueFamily {
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::FunctionReadability => "function_readability",
            Self::DocumentationIntegrity => "documentation_integrity",
            Self::ImplementationDuplication => "implementation_duplication",
            Self::DependencyTopology => "dependency_topology",
            Self::ModuleSurface => "module_surface",
            Self::BoundaryIntegrity => "boundary_integrity",
            Self::ResponsibilityDecomposition => "responsibility_decomposition",
            Self::DirectoryOrganization => "directory_organization",
            Self::LiteralOwnership => "literal_ownership",
            Self::DataShapeDuplication => "data_shape_duplication",
            Self::ErrorHandlingDuplication => "error_handling_duplication",
            Self::TestSupport => "test_support",
            Self::TestCoverage => "test_coverage",
            Self::DeadCode => "dead_code",
            Self::DeclaredDebt => "declared_debt",
            Self::Naming => "naming",
            Self::CompatibilityRetirement => "compatibility_retirement",
            Self::DataflowOwnership => "dataflow_ownership",
        }
    }

    pub(crate) fn qualified(self, analysis: &str) -> String {
        format!("reforge.{analysis}.{}", self.id())
    }

    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::FunctionReadability => "Function readability",
            Self::DocumentationIntegrity => "Documentation integrity",
            Self::ImplementationDuplication => "Implementation duplication",
            Self::DependencyTopology => "Dependency topology",
            Self::ModuleSurface => "Module surface",
            Self::BoundaryIntegrity => "Boundary integrity",
            Self::ResponsibilityDecomposition => "Responsibility decomposition",
            Self::DirectoryOrganization => "Directory organization",
            Self::LiteralOwnership => "Literal ownership",
            Self::DataShapeDuplication => "Data shape duplication",
            Self::ErrorHandlingDuplication => "Error handling duplication",
            Self::TestSupport => "Test support",
            Self::TestCoverage => "Test coverage",
            Self::DeadCode => "Dead code",
            Self::DeclaredDebt => "Declared debt",
            Self::Naming => "Naming",
            Self::CompatibilityRetirement => "Compatibility retirement",
            Self::DataflowOwnership => "Dataflow ownership",
        }
    }

    pub(crate) const fn guidance(self) -> &'static str {
        match self {
            Self::FunctionReadability => {
                "Reduce the function to a clear sequence of named responsibilities."
            }
            Self::DocumentationIntegrity => {
                "Update the active documentation so it matches the implemented contract."
            }
            Self::ImplementationDuplication => {
                "Consolidate shared behavior or make intentionally separate variants explicit."
            }
            Self::DependencyTopology => {
                "Reshape dependencies around stable, acyclic ownership boundaries."
            }
            Self::ModuleSurface => {
                "Narrow the module's dependencies and public surface to its owned responsibility."
            }
            Self::BoundaryIntegrity => "Route access and value flow through the declared boundary.",
            Self::ResponsibilityDecomposition => {
                "Split the subject around cohesive responsibilities."
            }
            Self::DirectoryOrganization => {
                "Organize files into directories with explicit conceptual ownership."
            }
            Self::LiteralOwnership => "Give repeated or drifting values one named owner.",
            Self::DataShapeDuplication => {
                "Introduce one owned data shape with explicit conversions at boundaries."
            }
            Self::ErrorHandlingDuplication => "Centralize the repeated error-handling policy.",
            Self::TestSupport => "Consolidate shared test setup while keeping assertions explicit.",
            Self::TestCoverage => "Add focused failure and boundary cases for the same behavior.",
            Self::DeadCode => "Remove the unused path or restore its intended caller.",
            Self::DeclaredDebt => {
                "Resolve the debt or record an owner, rationale, and tracking reference."
            }
            Self::Naming => "Use one naming convention within the owning scope.",
            Self::CompatibilityRetirement => {
                "Remove the compatibility path or record an explicit sunset plan."
            }
            Self::DataflowOwnership => "Assign the value flow an explicit owner or coordinator.",
        }
    }
}

include!("manifest/policy.rs");
