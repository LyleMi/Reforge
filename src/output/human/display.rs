#[derive(Debug, Default)]
struct FindingBreakdown {
    critical: usize,
    warnings: usize,
    info: usize,
    by_kind: BTreeMap<FindingKind, usize>,
}

impl FindingBreakdown {
    fn from_findings(findings: &[Finding]) -> Self {
        let mut breakdown = Self::default();

        for finding in findings {
            match finding.severity {
                Severity::Critical => breakdown.critical += 1,
                Severity::Warning => breakdown.warnings += 1,
                Severity::Info => breakdown.info += 1,
            }

            *breakdown.by_kind.entry(finding.kind).or_insert(0) += 1;
        }

        breakdown
    }

    fn count(&self, kind: FindingKind) -> usize {
        self.by_kind.get(&kind).copied().unwrap_or(0)
    }
}

#[derive(Debug, Clone, Copy)]
enum DisplayMetric {
    Primary,
    GroupSize,
    Named(MetricId),
}

#[derive(Debug, Clone, Copy)]
enum MetricFormat {
    Count(&'static str),
    PluralCount(&'static str),
    PrefixedPluralCount {
        prefix: &'static str,
        noun: &'static str,
    },
    NamedValue(&'static str),
}

impl MetricFormat {
    fn render(self, label: &str, value: usize) -> String {
        match self {
            Self::Count(unit) => format!("{label}: {value} {unit}"),
            Self::PluralCount(noun) => {
                format!("{label}: {value} {}", pluralize(value, noun))
            }
            Self::PrefixedPluralCount { prefix, noun } => {
                format!("{label}: {value} {prefix}{}", pluralize(value, noun))
            }
            Self::NamedValue(name) => format!("{label}: {name} {value}"),
        }
    }
}

fn pluralize(count: usize, noun: &str) -> String {
    if count == 1 {
        noun.to_string()
    } else {
        format!("{noun}s")
    }
}

#[derive(Debug, Clone, Copy)]
struct FindingKindDisplay {
    kind: FindingKind,
    label: &'static str,
    metric: Option<DisplayMetric>,
    format: MetricFormat,
}

const FINDING_KIND_DISPLAYS: &[FindingKindDisplay] = &[
    display(
        FindingKind::LargeFile,
        "large file",
        DisplayMetric::Primary,
        MetricFormat::Count("lines"),
    ),
    display(
        FindingKind::LargeDirectory,
        "large directory",
        DisplayMetric::Primary,
        MetricFormat::Count("source files"),
    ),
    FindingKindDisplay {
        kind: FindingKind::DebtMarker,
        label: "debt marker",
        metric: None,
        format: MetricFormat::Count(""),
    },
    display(
        FindingKind::SimilarFunctions,
        "similar functions",
        DisplayMetric::GroupSize,
        MetricFormat::PluralCount("function"),
    ),
    display(
        FindingKind::LongFunction,
        "long function",
        DisplayMetric::Primary,
        MetricFormat::Count("lines"),
    ),
    display(
        FindingKind::ComplexFunction,
        "complex function",
        DisplayMetric::Primary,
        MetricFormat::NamedValue("complexity"),
    ),
    display(
        FindingKind::DeepNesting,
        "deep nesting",
        DisplayMetric::Primary,
        MetricFormat::Count("levels"),
    ),
    display(
        FindingKind::ManyParameters,
        "many parameters",
        DisplayMetric::Primary,
        MetricFormat::Count("parameters"),
    ),
    display(
        FindingKind::ReadabilityRisk,
        "readability risk",
        DisplayMetric::Named(MetricId::ReadabilitySignalCount),
        MetricFormat::Count("signals"),
    ),
    display(
        FindingKind::LargeType,
        "large type",
        DisplayMetric::Named(MetricId::TypeLoc),
        MetricFormat::NamedValue("lines"),
    ),
    display(
        FindingKind::LargePublicSurface,
        "large public surface",
        DisplayMetric::Primary,
        MetricFormat::Count("items"),
    ),
    display(
        FindingKind::ImportHeavyFile,
        "import-heavy file",
        DisplayMetric::Primary,
        MetricFormat::Count("imports"),
    ),
    display(
        FindingKind::FunctionProliferation,
        "function proliferation",
        DisplayMetric::Named(MetricId::FileFunctionCount),
        MetricFormat::PluralCount("function"),
    ),
    display(
        FindingKind::UnusedFunction,
        "unused function",
        DisplayMetric::Named(MetricId::FunctionReferences),
        MetricFormat::Count("references"),
    ),
    display(
        FindingKind::RepeatedLiteral,
        "repeated literal",
        DisplayMetric::GroupSize,
        MetricFormat::Count("occurrences"),
    ),
    display(
        FindingKind::RepeatedErrorPattern,
        "repeated error pattern",
        DisplayMetric::GroupSize,
        MetricFormat::Count("occurrences"),
    ),
    display(
        FindingKind::TestDuplication,
        "test duplication",
        DisplayMetric::GroupSize,
        MetricFormat::Count("occurrences"),
    ),
    display(
        FindingKind::HappyPathOnlyTests,
        "happy-path-only tests",
        DisplayMetric::GroupSize,
        MetricFormat::PrefixedPluralCount {
            prefix: "test ",
            noun: "case",
        },
    ),
    display(
        FindingKind::FileNamingDrift,
        "file naming drift",
        DisplayMetric::GroupSize,
        MetricFormat::PrefixedPluralCount {
            prefix: "naming ",
            noun: "style",
        },
    ),
    display(
        FindingKind::DirectoryDrift,
        "directory drift",
        DisplayMetric::GroupSize,
        MetricFormat::Count("concepts"),
    ),
    display(
        FindingKind::DataClump,
        "data clump",
        DisplayMetric::GroupSize,
        MetricFormat::Count("occurrences"),
    ),
    display(
        FindingKind::ParallelImplementation,
        "parallel implementation",
        DisplayMetric::GroupSize,
        MetricFormat::PluralCount("implementation"),
    ),
    display(
        FindingKind::ShadowedAbstraction,
        "shadowed abstraction",
        DisplayMetric::GroupSize,
        MetricFormat::Count("occurrences"),
    ),
    display(
        FindingKind::DuplicateTypeShape,
        "duplicate type shape",
        DisplayMetric::GroupSize,
        MetricFormat::PluralCount("type shape"),
    ),
    display(
        FindingKind::ConfigKeyDrift,
        "config key drift",
        DisplayMetric::GroupSize,
        MetricFormat::PrefixedPluralCount {
            prefix: "config ",
            noun: "key",
        },
    ),
    display(
        FindingKind::FixtureFactoryDrift,
        "fixture factory drift",
        DisplayMetric::GroupSize,
        MetricFormat::PluralCount("factory"),
    ),
    display(
        FindingKind::GenericBucketDrift,
        "generic bucket drift",
        DisplayMetric::GroupSize,
        MetricFormat::Count("concepts"),
    ),
    display(
        FindingKind::AdapterBoundaryBypass,
        "adapter boundary bypass",
        DisplayMetric::GroupSize,
        MetricFormat::PluralCount("bypass"),
    ),
    display(
        FindingKind::StaleCompatibilityPath,
        "stale compatibility path",
        DisplayMetric::GroupSize,
        MetricFormat::PluralCount("marker"),
    ),
    display(
        FindingKind::MissingDocumentationSet,
        "missing documentation set",
        DisplayMetric::Primary,
        MetricFormat::PluralCount("missing required doc"),
    ),
    display(
        FindingKind::MissingUserGuide,
        "missing user guide",
        DisplayMetric::Primary,
        MetricFormat::PluralCount("missing user topic"),
    ),
    display(
        FindingKind::MissingReportSchemaDocs,
        "missing report schema docs",
        DisplayMetric::Primary,
        MetricFormat::Count("risk"),
    ),
    display(
        FindingKind::MissingMetricsModelDocs,
        "missing metrics model docs",
        DisplayMetric::Primary,
        MetricFormat::Count("risk"),
    ),
    display(
        FindingKind::MissingArchitectureDocs,
        "missing architecture docs",
        DisplayMetric::Primary,
        MetricFormat::Count("risk"),
    ),
    display(
        FindingKind::StaleCliDocumentation,
        "stale CLI documentation",
        DisplayMetric::Primary,
        MetricFormat::PluralCount("missing flag"),
    ),
    display(
        FindingKind::StaleSchemaDocumentation,
        "stale schema documentation",
        DisplayMetric::Primary,
        MetricFormat::PluralCount("missing field"),
    ),
    display(
        FindingKind::DependencyCycle,
        "dependency cycle",
        DisplayMetric::Named(MetricId::DependencyCycleFiles),
        MetricFormat::PluralCount("file"),
    ),
    display(
        FindingKind::DependencyHub,
        "dependency hub",
        DisplayMetric::Named(MetricId::DependencyFanOut),
        MetricFormat::Count("outgoing dependencies"),
    ),
];

const fn display(
    kind: FindingKind,
    label: &'static str,
    metric: DisplayMetric,
    format: MetricFormat,
) -> FindingKindDisplay {
    FindingKindDisplay {
        kind,
        label,
        metric: Some(metric),
        format,
    }
}

fn display_for_kind(kind: FindingKind) -> &'static FindingKindDisplay {
    FINDING_KIND_DISPLAYS
        .iter()
        .find(|display| display.kind == kind)
        .expect("every finding kind should have display metadata")
}

enum AnsiStyle {
    Header,
    Section,
    Path,
    Location,
    Muted,
    Warning,
    Critical,
    Info,
}

fn render_status_cell(status: BaselineIssueStatus, color: bool) -> String {
    paint(
        color,
        &format!("{:<8}", baseline_status_label(status)),
        match status {
            BaselineIssueStatus::New => AnsiStyle::Info,
            BaselineIssueStatus::Worse => AnsiStyle::Warning,
            BaselineIssueStatus::Same => AnsiStyle::Muted,
        },
    )
}

fn render_severity_cell(severity: Severity, color: bool) -> String {
    paint(
        color,
        &format!("{:<8}", severity.to_string()),
        match severity {
            Severity::Critical => AnsiStyle::Critical,
            Severity::Warning => AnsiStyle::Warning,
            Severity::Info => AnsiStyle::Info,
        },
    )
}

fn baseline_status_label(status: BaselineIssueStatus) -> &'static str {
    match status {
        BaselineIssueStatus::New => "new",
        BaselineIssueStatus::Worse => "worse",
        BaselineIssueStatus::Same => "same",
    }
}

fn baseline_show_label(show: BaselineShow) -> &'static str {
    match show {
        BaselineShow::New => "new",
        BaselineShow::NewOrWorse => "new or worse",
        BaselineShow::All => "all current",
    }
}

fn baseline_show_value(show: BaselineShow) -> &'static str {
    match show {
        BaselineShow::New => "new",
        BaselineShow::NewOrWorse => "new-or-worse",
        BaselineShow::All => "all",
    }
}

fn paint(color: bool, text: &str, style: AnsiStyle) -> String {
    if !color {
        return text.to_string();
    }

    let code = match style {
        AnsiStyle::Header => "1;36",
        AnsiStyle::Section => "1",
        AnsiStyle::Path => "36",
        AnsiStyle::Location => "2",
        AnsiStyle::Muted => "2",
        AnsiStyle::Critical => "1;31",
        AnsiStyle::Warning => "1;33",
        AnsiStyle::Info => "1;34",
    };

    format!("\x1b[{code}m{text}\x1b[0m")
}

fn format_duration(duration_ms: u128) -> String {
    if duration_ms < 1_000 {
        format!("{duration_ms} ms")
    } else {
        format!("{:.2} s", duration_ms as f64 / 1_000.0)
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info => write!(f, "info"),
            Severity::Warning => write!(f, "warning"),
            Severity::Critical => write!(f, "critical"),
        }
    }
}
