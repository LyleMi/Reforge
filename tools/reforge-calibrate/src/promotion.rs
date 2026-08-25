use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use reforge_engine::api::Analysis;
use sha2::{Digest, Sha256};

use crate::corpus::{corpus_digest, load_corpus};
use crate::io::read_json;
use crate::model::{CalibrationSummary, GroupSummary, PromotionVerification, ReportAudit};

pub(crate) fn verify_promotion(
    corpus_path: &Path,
    audit_paths: &[PathBuf],
    summary_paths: &[PathBuf],
) -> Result<PromotionVerification> {
    let manifest = load_corpus(corpus_path)?;
    let expected_corpus_digest = corpus_digest(corpus_path)?;
    let audits = audit_paths
        .iter()
        .map(|path| read_json::<ReportAudit>(path))
        .collect::<Result<Vec<_>>>()?;
    let report_digest = validate_audits(&manifest, &expected_corpus_digest, &audits)?;
    let summaries = summary_paths
        .iter()
        .map(|path| read_json::<CalibrationSummary>(path))
        .collect::<Result<Vec<_>>>()?;
    validate_summaries(
        &summaries,
        &expected_corpus_digest,
        report_digest.as_deref(),
    )?;

    let promoted = promoted_rule_languages();
    validate_promoted_evidence(&promoted, &summaries)?;

    Ok(PromotionVerification {
        verification_schema_version: 1,
        corpus_digest: expected_corpus_digest,
        audited_repositories: audits.len(),
        promotion_candidates: promoted.len(),
    })
}

fn validate_promoted_evidence(
    promoted: &[(String, String)],
    summaries: &[CalibrationSummary],
) -> Result<()> {
    let eligible = summaries
        .iter()
        .flat_map(|summary| &summary.groups)
        .filter(|group| group.eligible_for_stable_advisory && group.failures.is_empty())
        .map(|group| ((group.rule.as_str(), group.language.as_str()), group))
        .collect::<BTreeMap<_, _>>();
    for (rule, language) in promoted {
        let group = eligible
            .get(&(rule.as_str(), language.as_str()))
            .with_context(|| {
                format!(
                    "promoted rule `{rule}` language `{language}` lacks eligible calibration evidence"
                )
            })?;
        validate_group_thresholds(group)?;
    }
    Ok(())
}

fn validate_audits(
    manifest: &crate::model::CorpusManifest,
    corpus_digest: &str,
    audits: &[ReportAudit],
) -> Result<Option<String>> {
    if audits.is_empty() {
        return Ok(None);
    }
    if audits.len() != manifest.repositories.len() {
        bail!(
            "promotion verification requires {} corpus audits, found {}",
            manifest.repositories.len(),
            audits.len()
        );
    }
    let expected = manifest
        .repositories
        .iter()
        .map(|entry| (entry.repository.as_str(), entry.commit.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    for audit in audits {
        validate_audit(audit, corpus_digest, &expected)?;
        if !seen.insert(audit.repository.as_str()) {
            bail!("duplicate report audit for `{}`", audit.repository);
        }
    }
    Ok(Some(audit_set_digest(audits)))
}

fn validate_audit(
    audit: &ReportAudit,
    corpus_digest: &str,
    expected: &BTreeMap<&str, &str>,
) -> Result<()> {
    if audit.audit_schema_version != 1
        || audit.report_schema != reforge_schema::REPORT_SCHEMA_VERSION
    {
        bail!("unsupported report audit schema");
    }
    if audit.corpus_digest != corpus_digest {
        bail!(
            "report audit for `{}` is bound to a different corpus digest",
            audit.repository
        );
    }
    if expected.get(audit.repository.as_str()).copied() != Some(audit.revision.as_str()) {
        bail!(
            "report audit for `{}` does not match its frozen revision",
            audit.repository
        );
    }
    validate_sha256(&audit.report_digest).with_context(|| {
        format!(
            "report audit for `{}` has an invalid digest",
            audit.repository
        )
    })
}

fn validate_sha256(value: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        bail!("expected a lowercase SHA-256 digest");
    }
    Ok(())
}

fn audit_set_digest(audits: &[ReportAudit]) -> String {
    let mut values = audits
        .iter()
        .map(|audit| (&audit.repository, &audit.report_digest))
        .collect::<Vec<_>>();
    values.sort();
    let mut digest = Sha256::new();
    for (repository, report_digest) in values {
        digest.update(repository.as_bytes());
        digest.update([0]);
        digest.update(report_digest.as_bytes());
        digest.update([0]);
    }
    hex::encode(digest.finalize())
}

fn validate_summaries(
    summaries: &[CalibrationSummary],
    corpus_digest: &str,
    report_digest: Option<&str>,
) -> Result<()> {
    for summary in summaries {
        if summary.summary_schema_version != 1
            || summary.validation_basis != "automated"
            || summary.reviewers.len() < 2
        {
            bail!("calibration summary lacks the required isolated review provenance");
        }
        if summary.corpus_digest != corpus_digest {
            bail!("calibration summary is bound to a different corpus digest");
        }
        if report_digest != Some(summary.report_digest.as_str()) {
            bail!("calibration summary is bound to a different report audit digest");
        }
    }
    Ok(())
}

fn promoted_rule_languages() -> Vec<(String, String)> {
    let analyses = BTreeSet::from([Analysis::Codebase, Analysis::Dataflow]);
    reforge_engine::api::rules(&analyses)
        .into_iter()
        .filter(|rule| {
            rule["maturity"].as_str() == Some("stable")
                || rule["default_enabled"].as_bool() == Some(true)
        })
        .flat_map(|rule| {
            let rule_id = rule["rule"].as_str().unwrap_or_default().to_owned();
            rule["languages"]
                .as_object()
                .into_iter()
                .flat_map(|languages| languages.keys())
                .map(move |language| (rule_id.clone(), language.clone()))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn validate_group_thresholds(group: &GroupSummary) -> Result<()> {
    if group.candidate_sites < 40
        || group.candidate_repositories < 5
        || group.max_repository_share > 0.25
        || group.quiet_sites < 20
        || group.quiet_repositories < 3
        || group.fixture_recall.total == 0
        || group.fixture_recall.successes as f64 / (group.fixture_recall.total as f64) < 0.90
        || group.unsupported_coverage.total == 0
        || group.unsupported_coverage.successes != group.unsupported_coverage.total
    {
        bail!(
            "calibration group `{}`/`{}` does not satisfy promotion thresholds",
            group.rule,
            group.language
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_registry_has_no_promotion_candidates() {
        assert!(promoted_rule_languages().is_empty());
    }

    #[test]
    fn promoted_rule_without_evidence_fails_closed() {
        assert!(
            validate_promoted_evidence(
                &[("reforge.codebase.future_rule".into(), "rust".into())],
                &[],
            )
            .is_err()
        );
    }

    #[test]
    fn forged_audit_digests_fail_closed() {
        let expected = BTreeMap::from([("owner/repository", "revision")]);
        let mut audit = ReportAudit {
            audit_schema_version: 1,
            report_schema: reforge_schema::REPORT_SCHEMA_VERSION,
            corpus_digest: "corpus-digest".into(),
            repository: "owner/repository".into(),
            revision: "revision".into(),
            workspace_identity: "workspace".into(),
            report_digest: "0".repeat(64),
            artifacts: BTreeMap::new(),
            coverage_status: BTreeMap::new(),
        };

        audit.corpus_digest = "forged-corpus-digest".into();
        assert!(validate_audit(&audit, "corpus-digest", &expected).is_err());

        audit.corpus_digest = "corpus-digest".into();
        audit.report_digest = "not-a-sha256".into();
        assert!(validate_audit(&audit, "corpus-digest", &expected).is_err());
    }
}
