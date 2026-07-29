use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, bail};
use reforge_schema::{ANALYSIS_CODEBASE, ANALYSIS_DATAFLOW, Report};
use sha2::{Digest, Sha256};

use crate::cli::VerifyReportsArgs;
use crate::corpus::{corpus_digest, load_corpus};
use crate::io::{sha256_file, write_json};
use crate::model::ReportAudit;

pub(crate) fn verify_reports(args: &VerifyReportsArgs) -> Result<()> {
    let manifest = load_corpus(&args.manifest)?;
    let entry = manifest
        .repositories
        .iter()
        .find(|entry| entry.repository == args.repository)
        .with_context(|| format!("repository `{}` is absent from corpus", args.repository))?;
    if entry.commit != args.revision {
        bail!(
            "revision {} does not match frozen corpus commit {}",
            args.revision,
            entry.commit
        );
    }
    verify_repeat("Codebase", &args.codebase, &args.codebase_repeat)?;
    verify_repeat("Dataflow", &args.dataflow, &args.dataflow_repeat)?;
    verify_repeat("combined", &args.combined, &args.combined_repeat)?;
    verify_repeat("metrics sidecar", &args.metrics, &args.metrics_repeat)?;
    verify_repeat("Flow IR sidecar", &args.flow_ir, &args.flow_ir_repeat)?;
    validate_sidecar("metrics", &args.metrics)?;
    validate_sidecar("Flow IR", &args.flow_ir)?;

    let codebase = reforge_output::load_report(&args.codebase)?;
    let dataflow = reforge_output::load_report(&args.dataflow)?;
    let combined = reforge_output::load_report(&args.combined)?;
    validate_report_set(&codebase, &dataflow, &combined, &args.revision)?;

    let artifacts = artifact_digests(args)?;
    let audit = ReportAudit {
        audit_schema_version: 1,
        report_schema: reforge_schema::REPORT_SCHEMA_VERSION,
        corpus_digest: corpus_digest(&args.manifest)?,
        repository: args.repository.clone(),
        revision: args.revision.clone(),
        workspace_identity: combined.target.workspace_identity.clone(),
        report_digest: aggregate_digest(&artifacts),
        artifacts,
        coverage_status: combined
            .coverage
            .iter()
            .map(|(analysis, coverage)| {
                (
                    analysis.clone(),
                    format!("{:?}", coverage.status).to_ascii_lowercase(),
                )
            })
            .collect(),
    };
    write_json(&args.output, &audit)
}

fn verify_repeat(label: &str, first: &Path, second: &Path) -> Result<()> {
    let first_bytes =
        std::fs::read(first).with_context(|| format!("failed to read {}", first.display()))?;
    let second_bytes =
        std::fs::read(second).with_context(|| format!("failed to read {}", second.display()))?;
    if first_bytes != second_bytes {
        bail!("{label} reproducible reports are not byte-identical");
    }
    Ok(())
}

fn validate_sidecar(label: &str, path: &Path) -> Result<()> {
    let bytes =
        std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    if bytes.is_empty() {
        bail!("{label} sidecar is empty");
    }
    serde_json::from_slice::<serde_json::Value>(&bytes)
        .with_context(|| format!("{label} sidecar is not valid JSON"))?;
    Ok(())
}

fn validate_report_set(
    codebase: &Report,
    dataflow: &Report,
    combined: &Report,
    revision: &str,
) -> Result<()> {
    validate_coverage_keys(codebase, BTreeSet::from([ANALYSIS_CODEBASE]))?;
    validate_coverage_keys(dataflow, BTreeSet::from([ANALYSIS_DATAFLOW]))?;
    validate_coverage_keys(
        combined,
        BTreeSet::from([ANALYSIS_CODEBASE, ANALYSIS_DATAFLOW]),
    )?;
    validate_report_targets(codebase, dataflow, combined, revision)?;
    validate_combined_coverage(codebase, dataflow, combined)?;
    validate_issue_union(codebase, dataflow, combined)
}

fn validate_report_targets(
    codebase: &Report,
    dataflow: &Report,
    combined: &Report,
    revision: &str,
) -> Result<()> {
    for report in [codebase, dataflow, combined] {
        if report.target.source_revision.as_deref() != Some(revision) {
            bail!("report source revision does not match frozen corpus revision");
        }
        if report.target.workspace_identity.trim().is_empty() {
            bail!("report workspace identity must not be empty");
        }
    }
    if codebase.target.workspace_identity != dataflow.target.workspace_identity
        || codebase.target.workspace_identity != combined.target.workspace_identity
    {
        bail!("isolated and combined reports name different workspaces");
    }
    Ok(())
}

fn validate_combined_coverage(
    codebase: &Report,
    dataflow: &Report,
    combined: &Report,
) -> Result<()> {
    if combined.coverage.get(ANALYSIS_CODEBASE) != codebase.coverage.get(ANALYSIS_CODEBASE)
        || combined.coverage.get(ANALYSIS_DATAFLOW) != dataflow.coverage.get(ANALYSIS_DATAFLOW)
    {
        bail!("combined report coverage does not match isolated analysis coverage");
    }
    Ok(())
}

fn validate_issue_union(codebase: &Report, dataflow: &Report, combined: &Report) -> Result<()> {
    let mut isolated = issue_map(codebase)?;
    for (id, issue) in issue_map(dataflow)? {
        if isolated.insert(id.clone(), issue).is_some() {
            bail!("isolated reports contain duplicate issue ID {id}");
        }
    }
    if isolated != issue_map(combined)? {
        bail!("combined report issues do not equal the isolated analysis union");
    }
    Ok(())
}

fn validate_coverage_keys(report: &Report, expected: BTreeSet<&str>) -> Result<()> {
    let actual = report
        .coverage
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if actual != expected {
        bail!("report analysis isolation mismatch: expected {expected:?}, found {actual:?}");
    }
    Ok(())
}

fn issue_map(report: &Report) -> Result<BTreeMap<String, serde_json::Value>> {
    report
        .issues
        .iter()
        .map(|issue| Ok((issue.id.clone(), serde_json::to_value(issue)?)))
        .collect()
}

fn artifact_digests(args: &VerifyReportsArgs) -> Result<BTreeMap<String, String>> {
    [
        ("codebase.json", &args.codebase),
        ("codebase-repeat.json", &args.codebase_repeat),
        ("dataflow.json", &args.dataflow),
        ("dataflow-repeat.json", &args.dataflow_repeat),
        ("combined.json", &args.combined),
        ("combined-repeat.json", &args.combined_repeat),
        ("metrics.json", &args.metrics),
        ("metrics-repeat.json", &args.metrics_repeat),
        ("flow-ir.json", &args.flow_ir),
        ("flow-ir-repeat.json", &args.flow_ir_repeat),
    ]
    .into_iter()
    .map(|(name, path)| Ok((name.into(), sha256_file(path)?)))
    .collect()
}

fn aggregate_digest(artifacts: &BTreeMap<String, String>) -> String {
    let mut digest = Sha256::new();
    for (name, value) in artifacts {
        digest.update(name.as_bytes());
        digest.update([0]);
        digest.update(value.as_bytes());
        digest.update([0]);
    }
    format!("{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use reforge_schema::{
        AnalysisCoverage, CoverageStatus, Producer, ReportInput, SuppressionSummary, Target,
        default_provenance,
    };

    #[test]
    fn aggregate_digest_changes_with_artifact_digest() {
        let first = BTreeMap::from([("report".into(), "a".into())]);
        let second = BTreeMap::from([("report".into(), "b".into())]);
        assert_ne!(aggregate_digest(&first), aggregate_digest(&second));
    }

    #[test]
    fn non_deterministic_report_pair_is_rejected() {
        let root =
            std::env::temp_dir().join(format!("reforge-calibrate-repeat-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let first = root.join("first.json");
        let second = root.join("second.json");
        std::fs::write(&first, b"{\"value\":1}").unwrap();
        std::fs::write(&second, b"{\"value\":2}").unwrap();

        assert!(verify_repeat("Codebase", &first, &second).is_err());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn combined_coverage_must_equal_isolated_coverage() {
        let codebase = report(BTreeMap::from([(ANALYSIS_CODEBASE.into(), coverage(1))]));
        let dataflow = report(BTreeMap::from([(ANALYSIS_DATAFLOW.into(), coverage(1))]));
        let combined = report(BTreeMap::from([
            (ANALYSIS_CODEBASE.into(), coverage(2)),
            (ANALYSIS_DATAFLOW.into(), coverage(1)),
        ]));

        assert!(validate_report_set(&codebase, &dataflow, &combined, "revision").is_err());
    }

    fn coverage(scanned_files: usize) -> AnalysisCoverage {
        AnalysisCoverage {
            status: CoverageStatus::Observed,
            scanned_files,
            languages: BTreeMap::new(),
            rules: BTreeMap::new(),
            limitations: Vec::new(),
        }
    }

    fn report(coverage: BTreeMap<String, AnalysisCoverage>) -> Report {
        let issues = Vec::new();
        Report::new(ReportInput {
            producer: Producer {
                name: "reforge.analyze".into(),
                version: "test".into(),
                revision: None,
            },
            target: Target {
                root: "/tmp/repository".into(),
                workspace_identity: "workspace".into(),
                source_revision: Some("revision".into()),
            },
            provenance: default_provenance(&coverage, &issues),
            suppression: SuppressionSummary::default(),
            coverage,
            issues,
        })
    }
}
