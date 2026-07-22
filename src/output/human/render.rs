fn render_cluster_context(cluster: &Issue, color: bool) -> String {
    let kinds = cluster
        .kinds
        .iter()
        .map(|kind| crate::model::serialized_finding_kind(*kind))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "            {} {} raw signals ({kinds})\n",
        paint(color, "cluster", AnsiStyle::Muted),
        cluster.finding_ids.len()
    )
}

fn sorted_findings(findings: &[Finding]) -> Vec<&Finding> {
    let mut sorted = findings.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.id.cmp(&right.id))
    });
    sorted
}

fn sorted_baseline_issues<'a>(issues: &'a [BaselineIssue<'a>]) -> Vec<&'a BaselineIssue<'a>> {
    let mut sorted = issues.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| {
        left.issue
            .path
            .cmp(&right.issue.path)
            .then_with(|| left.issue.line.cmp(&right.issue.line))
            .then_with(|| left.issue.id.cmp(&right.issue.id))
    });
    sorted
}

fn render_finding(finding: &Finding, color: bool) -> String {
    let location = finding
        .line
        .map(|line| format!("{}:{line}", display_path(&finding.path)))
        .unwrap_or_else(|| display_path(&finding.path));
    let metrics = render_metric_summary(finding);

    let mut output = format!(
        "  {}  {}\n            {}\n",
        paint(
            color,
            &crate::model::serialized_finding_kind(finding.kind),
            AnsiStyle::Info
        ),
        concise_finding_message(finding),
        paint(color, &location, AnsiStyle::Path),
    );

    if let Some(metrics) = metrics {
        output.push_str(&format!("            metrics {metrics}\n"));
    }
    output.push_str(&format!("            hint {}\n", finding.recommendation()));

    output
}
fn render_diff_issue(entry: &BaselineIssue<'_>, color: bool) -> String {
    let issue = entry.issue;
    let location = issue
        .line
        .map(|line| format!("{}:{line}", display_path(&issue.path)))
        .unwrap_or_else(|| display_path(&issue.path));

    format!(
        "  {} {}\n            {}\n            evidence {}\n",
        render_status_cell(entry.status, color),
        issue.summary,
        paint(color, &location, AnsiStyle::Path),
        issue.finding_ids.len(),
    )
}

fn display_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let Ok(current_dir) = std::env::current_dir() else {
        return normalized;
    };
    let current_dir = current_dir.to_string_lossy().replace('\\', "/");
    let current_dir = current_dir.trim_end_matches('/');

    if normalized == current_dir {
        return ".".to_string();
    }

    normalized
        .strip_prefix(&format!("{current_dir}/"))
        .unwrap_or(&normalized)
        .to_string()
}

fn render_metric_summary(finding: &Finding) -> Option<String> {
    if finding.metrics.is_empty() {
        return None;
    }

    Some(
        finding
            .metrics
            .iter()
            .map(|metric| {
                let value = if let Some(threshold) = metric.threshold {
                    format!("{}={}/{}", metric.name, metric.value, threshold)
                } else {
                    format!("{}={}", metric.name, metric.value)
                };

                if metric.unit.is_empty() {
                    value
                } else {
                    format!("{value} {}", metric.unit)
                }
            })
            .collect::<Vec<_>>()
            .join(", "),
    )
}

fn render_suppression_summary(report: &ScanReport) -> String {
    let summary = &report.suppression_summary;
    format!("{} findings", summary.suppressed_count)
}

fn concise_finding_message(finding: &Finding) -> String {
    if !finding.message.is_empty() && finding.kind != FindingKind::DebtMarker {
        return finding.message.clone();
    }

    let display = display_for_kind(finding.kind);
    render_kind_metric_message(finding, display).unwrap_or_else(|| display.label.to_string())
}

fn render_kind_metric_message(finding: &Finding, display: &FindingKindDisplay) -> Option<String> {
    let value = match display.metric {
        Some(DisplayMetric::Primary) => primary_metric_value(finding),
        Some(DisplayMetric::GroupSize) => group_size(finding),
        Some(DisplayMetric::Named(name)) => metric_value(finding, name.as_str()),
        None => None,
    }?;
    Some(display.format.render(display.label, value))
}

fn metric_value(finding: &Finding, name: &str) -> Option<usize> {
    finding
        .metrics
        .iter()
        .find(|metric| metric.name.as_str() == name)
        .map(|metric| metric.value)
}

fn primary_metric_value(finding: &Finding) -> Option<usize> {
    finding.metrics.first().map(|metric| metric.value)
}

fn group_size(finding: &Finding) -> Option<usize> {
    metric_value(finding, MetricId::GroupSize.as_str()).or_else(|| primary_metric_value(finding))
}

fn has_related_location_details(finding: &Finding) -> bool {
    matches!(
        finding.kind,
        FindingKind::SimilarFunctions
            | FindingKind::RepeatedLiteral
            | FindingKind::RepeatedErrorPattern
            | FindingKind::TestDuplication
            | FindingKind::HappyPathOnlyTests
            | FindingKind::FileNamingDrift
            | FindingKind::DataClump
            | FindingKind::ParallelImplementation
            | FindingKind::ShadowedAbstraction
            | FindingKind::DuplicateTypeShape
            | FindingKind::ConfigKeyDrift
            | FindingKind::FixtureFactoryDrift
            | FindingKind::GenericBucketDrift
            | FindingKind::AdapterBoundaryBypass
            | FindingKind::AdapterFlowBypass
            | FindingKind::StaleCompatibilityPath
            | FindingKind::DependencyCycle
    )
}

fn render_related_locations(finding: &Finding, color: bool) -> String {
    let mut output = String::new();
    output.push_str("            related\n");

    for location in finding
        .related_locations
        .iter()
        .take(RELATED_LOCATION_LIMIT)
    {
        output.push_str("              ");
        output.push_str(&paint(
            color,
            &display_path(&location.path),
            AnsiStyle::Path,
        ));
        output.push_str(&paint(
            color,
            &format!(":{}", location.line),
            AnsiStyle::Location,
        ));
        if let Some(name) = &location.name {
            output.push(' ');
            output.push_str(name);
        }
        output.push('\n');
    }

    if finding.related_locations.len() > RELATED_LOCATION_LIMIT {
        output.push_str(&format!(
            "              +{} more\n",
            finding.related_locations.len() - RELATED_LOCATION_LIMIT
        ));
    }

    output
}
