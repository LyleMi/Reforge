fn write_human(mut writer: impl Write, report: &Report) -> Result<()> {
    writeln!(
        writer,
        "{} {} report (schema 26)",
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
    for issue in &report.issues {
        write_human_issue(&mut writer, issue)?;
    }
    writeln!(writer, "\nCoverage:")?;
    for (analysis, coverage) in &report.coverage {
        write_human_coverage(&mut writer, analysis, coverage)?;
    }
    Ok(())
}

fn write_human_issue(writer: &mut impl Write, issue: &reforge_schema::Issue) -> Result<()> {
    writeln!(writer, "\n{}  {}", issue.id, issue.title)?;
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
    for (rule, execution) in &coverage.rules {
        write_human_rule(writer, rule, execution)?;
    }
    Ok(())
}

fn write_human_rule(
    writer: &mut impl Write,
    rule: &str,
    execution: &reforge_schema::RuleExecution,
) -> Result<()> {
    if execution.status != reforge_schema::CoverageStatus::Observed {
        writeln!(writer, "    rule {rule}: {:?}", execution.status)?;
    }
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
