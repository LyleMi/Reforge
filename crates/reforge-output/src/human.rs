fn write_human(mut writer: impl Write, report: &Report) -> Result<()> {
    writeln!(
        writer,
        "{} {} report (schema 27)",
        report.producer.name, report.producer.version
    )?;
    writeln!(writer, "Target: {}", report.target.root)?;
    writeln!(
        writer,
        "Issues: {}  Evidence: {}  Suppressed: {}",
        report.summary.issue_count,
        report.summary.evidence_count,
        report.suppression.evidence_count
    )?;
    if report.issues.is_empty() {
        writeln!(writer, "No issues reported.")?;
    }
    let mut issues = report.issues.iter().collect::<Vec<_>>();
    issues.sort_by_key(|issue| issue_sort_key(report, issue));
    for issue in issues {
        write_human_issue(&mut writer, issue)?;
    }
    writeln!(writer, "\nCoverage:")?;
    for (analysis, coverage) in &report.coverage {
        write_human_coverage(&mut writer, analysis, coverage)?;
    }
    Ok(())
}

fn issue_sort_key(
    report: &Report,
    issue: &reforge_schema::Issue,
) -> (u8, u8, String, String, String, String) {
    let kind = match issue.kind {
        reforge_schema::IssueKind::Policy => 0,
        reforge_schema::IssueKind::Advisory => 1,
    };
    let baseline = report
        .baseline_comparison
        .as_ref()
        .and_then(|comparison| comparison.issues.get(&issue.id))
        .map(|entry| match entry.state {
            reforge_schema::BaselineState::New => 0,
            reforge_schema::BaselineState::Updated => 1,
            reforge_schema::BaselineState::Unknown => 2,
            reforge_schema::BaselineState::Unchanged => 3,
            reforge_schema::BaselineState::Absent => 4,
        })
        .unwrap_or(5);
    (
        kind,
        baseline,
        issue.analysis.clone(),
        issue.family.clone(),
        issue.subject.identity(),
        issue.id.clone(),
    )
}

fn write_human_issue(writer: &mut impl Write, issue: &reforge_schema::Issue) -> Result<()> {
    writeln!(writer, "\n{}  {}", issue.id, issue.title)?;
    writeln!(writer, "  kind: {:?}", issue.kind)?;
    writeln!(writer, "  family: {}", issue.family)?;
    writeln!(writer, "  subject: {}", issue.subject.display_name())?;
    writeln!(writer, "  guidance: {}", issue.guidance)?;
    for evidence in &issue.evidence {
        writeln!(writer, "  - {}: {}", evidence.rule, evidence.message)?;
    }
    Ok(())
}

fn write_human_coverage(
    writer: &mut impl Write,
    analysis: &str,
    coverage: &reforge_schema::AnalysisCoverage,
) -> Result<()> {
    writeln!(
        writer,
        "  {}: {:?} ({})",
        analysis, coverage.status, coverage.scanned_files
    )?;
    for limitation in &coverage.limitations {
        write_human_limitation(writer, "    ", limitation)?;
    }
    for (language, receipt) in &coverage.languages {
        writeln!(
            writer,
            "    language {language}: {:?} ({} files, {} functions)",
            receipt.status, receipt.files, receipt.functions
        )?;
        for limitation in &receipt.limitations {
            write_human_limitation(writer, "      ", limitation)?;
        }
        for (capability, capability_receipt) in &receipt.capabilities {
            writeln!(
                writer,
                "      capability {capability}: {:?}",
                capability_receipt.status
            )?;
            for limitation in &capability_receipt.limitations {
                write_human_limitation(writer, "        ", limitation)?;
            }
        }
    }
    write_human_rules(writer, coverage)?;
    Ok(())
}

fn write_human_rules(
    writer: &mut impl Write,
    coverage: &reforge_schema::AnalysisCoverage,
) -> Result<()> {
    for (rule, execution) in &coverage.rules {
        if execution.enabled_source != reforge_schema::RuleActivation::Disabled {
            write_human_rule(writer, rule, execution)?;
        }
    }
    let disabled = coverage
        .rules
        .values()
        .filter(|execution| {
            execution.enabled_source == reforge_schema::RuleActivation::Disabled
        })
        .count();
    if disabled > 0 {
        writeln!(
            writer,
            "    {disabled} disabled rule(s) omitted; JSON and YAML retain full coverage"
        )?;
    }
    Ok(())
}

fn write_human_rule(
    writer: &mut impl Write,
    rule: &str,
    execution: &reforge_schema::RuleExecution,
) -> Result<()> {
    writeln!(
        writer,
        "    rule {rule}: maturity={} source={:?} status={:?}",
        execution.maturity, execution.enabled_source, execution.status
    )?;
    for observation in &execution.observations {
        writeln!(
            writer,
            "      {}: {} {}",
            observation.name, observation.count, observation.unit
        )?;
    }
    for limitation in &execution.limitations {
        write_human_limitation(writer, "      ", limitation)?;
    }
    Ok(())
}

fn write_human_limitation(
    writer: &mut impl Write,
    indent: &str,
    limitation: &reforge_schema::CoverageLimitation,
) -> Result<()> {
    writeln!(
        writer,
        "{}{} ({}): {}",
        indent, limitation.code, limitation.count, limitation.message
    )?;
    Ok(())
}
