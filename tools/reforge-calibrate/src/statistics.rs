use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, bail};

use crate::io::{read_json, write_json};
use crate::model::{
    CalibrationSummary, DimensionSummary, GroupSummary, LabelFile, PacketSite, Rate, ReviewPacket,
    SiteLabel,
};
use crate::validation::{DIMENSIONS, validate_labels};

pub(crate) struct SummaryRequest<'a> {
    pub(crate) packet: &'a Path,
    pub(crate) reviewer_a: &'a Path,
    pub(crate) reviewer_b: &'a Path,
    pub(crate) adjudication: Option<&'a Path>,
    pub(crate) corpus_digest: String,
    pub(crate) report_digest: String,
    pub(crate) output: &'a Path,
}

pub(crate) fn summarize(request: SummaryRequest<'_>) -> Result<()> {
    validate_sha256("corpus", &request.corpus_digest)?;
    validate_sha256("report", &request.report_digest)?;
    let packet = read_json::<ReviewPacket>(request.packet)?;
    let reviewer_a = read_json::<LabelFile>(request.reviewer_a)?;
    let reviewer_b = read_json::<LabelFile>(request.reviewer_b)?;
    validate_labels(&packet, &reviewer_a)?;
    validate_labels(&packet, &reviewer_b)?;
    validate_reviewer_isolation(&reviewer_a, &reviewer_b)?;
    let adjudication = request
        .adjudication
        .map(read_json::<LabelFile>)
        .transpose()?;
    if let Some(labels) = &adjudication {
        validate_labels(&packet, labels)?;
    }
    let a = labels_by_site(&reviewer_a);
    let b = labels_by_site(&reviewer_b);
    let final_labels = adjudication.as_ref().map(labels_by_site);
    let groups = grouped_sites(&packet)
        .into_iter()
        .map(|((rule, language), sites)| {
            group_summary(GroupInputs {
                rule,
                language,
                sites: &sites,
                reviewer_a: &a,
                reviewer_b: &b,
                adjudicated: final_labels.as_ref(),
            })
        })
        .collect();
    let mut reviewers = vec![reviewer_a.reviewer, reviewer_b.reviewer];
    if let Some(labels) = adjudication {
        reviewers.push(labels.reviewer);
    }
    write_json(
        request.output,
        &CalibrationSummary {
            summary_schema_version: 1,
            validation_basis: "automated".into(),
            packet_digest: packet.packet_digest,
            corpus_digest: request.corpus_digest,
            report_digest: request.report_digest,
            reviewers,
            groups,
        },
    )
}

fn validate_sha256(label: &str, digest: &str) -> Result<()> {
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        bail!("{label} digest must be a lowercase SHA-256");
    }
    Ok(())
}

fn validate_reviewer_isolation(a: &LabelFile, b: &LabelFile) -> Result<()> {
    if a.reviewer.model == b.reviewer.model
        && a.reviewer.version == b.reviewer.version
        && a.reviewer.prompt_digest == b.reviewer.prompt_digest
    {
        bail!("reviewers must be isolated model/prompt executions");
    }
    Ok(())
}

fn grouped_sites(packet: &ReviewPacket) -> BTreeMap<(&str, &str), Vec<&PacketSite>> {
    let mut grouped = BTreeMap::new();
    for site in &packet.sites {
        grouped
            .entry((site.rule.as_str(), site.language.as_str()))
            .or_insert_with(Vec::new)
            .push(site);
    }
    grouped
}

struct GroupInputs<'a> {
    rule: &'a str,
    language: &'a str,
    sites: &'a [&'a PacketSite],
    reviewer_a: &'a BTreeMap<&'a str, &'a SiteLabel>,
    reviewer_b: &'a BTreeMap<&'a str, &'a SiteLabel>,
    adjudicated: Option<&'a BTreeMap<&'a str, &'a SiteLabel>>,
}

fn group_summary(input: GroupInputs<'_>) -> GroupSummary {
    let candidates = input
        .sites
        .iter()
        .copied()
        .filter(|site| site.candidate)
        .collect::<Vec<_>>();
    let quiet = input
        .sites
        .iter()
        .copied()
        .filter(|site| site.quiet)
        .collect::<Vec<_>>();
    let candidate_repositories = repository_count(&candidates);
    let quiet_repositories = repository_count(&quiet);
    let max_repository_share = max_repository_share(&candidates);
    let dimensions = dimension_summaries(&input);
    let fixture = fixture_rate(&input);
    let unsupported = unsupported_rate(&input);
    let failures = threshold_failures(ThresholdInputs {
        candidates: &candidates,
        candidate_repositories,
        max_repository_share,
        quiet: &quiet,
        quiet_repositories,
        dimensions: &dimensions,
        fixture,
        unsupported,
    });
    GroupSummary {
        rule: input.rule.into(),
        language: input.language.into(),
        candidate_sites: candidates.len(),
        candidate_repositories,
        max_repository_share,
        quiet_sites: quiet.len(),
        quiet_repositories,
        fixture_recall: fixture,
        unsupported_coverage: unsupported,
        eligible_for_stable_advisory: failures.is_empty(),
        dimensions,
        failures,
    }
}

fn dimension_summaries(input: &GroupInputs<'_>) -> BTreeMap<String, DimensionSummary> {
    DIMENSIONS
        .into_iter()
        .map(|dimension| {
            let pairs = judgment_pairs(input, dimension);
            let positives = pairs.iter().filter(|(_, _, value)| *value).count();
            (
                dimension.into(),
                DimensionSummary {
                    positive_rate: rate(positives, pairs.len()),
                    raw_agreement: agreement(&pairs),
                    cohens_kappa: kappa(&pairs),
                },
            )
        })
        .collect()
}

fn judgment_pairs(input: &GroupInputs<'_>, dimension: &str) -> Vec<(bool, bool, bool)> {
    input
        .sites
        .iter()
        .map(|site| {
            let a = input.reviewer_a[site.id.as_str()].judgments[dimension];
            let b = input.reviewer_b[site.id.as_str()].judgments[dimension];
            let final_value = input
                .adjudicated
                .map(|labels| labels[site.id.as_str()].judgments[dimension])
                .unwrap_or(a && b);
            (a, b, final_value)
        })
        .collect()
}

struct ThresholdInputs<'a> {
    candidates: &'a [&'a PacketSite],
    candidate_repositories: usize,
    max_repository_share: f64,
    quiet: &'a [&'a PacketSite],
    quiet_repositories: usize,
    dimensions: &'a BTreeMap<String, DimensionSummary>,
    fixture: Rate,
    unsupported: Rate,
}

fn threshold_failures(input: ThresholdInputs<'_>) -> Vec<String> {
    [
        (
            candidate_distribution_failed(&input),
            "candidate distribution threshold not met",
        ),
        (
            quiet_distribution_failed(&input),
            "quiet/negative distribution threshold not met",
        ),
        (
            dimension_lower_bound_failed(&input, "detection_claim_correctness", 0.90),
            "detection correctness Wilson lower bound below 0.90",
        ),
        (
            dimension_lower_bound_failed(&input, "useful_for_inspection", 0.80),
            "usefulness Wilson lower bound below 0.80",
        ),
        (
            reviewer_agreement_failed(&input),
            "reviewer agreement threshold not met",
        ),
        (fixture_recall_failed(&input), "fixture recall below 0.90"),
        (
            unsupported_coverage_failed(&input),
            "unsupported coverage is not 100% honest",
        ),
    ]
    .into_iter()
    .filter(|(failed, _)| *failed)
    .map(|(_, message)| message.into())
    .collect()
}

fn candidate_distribution_failed(input: &ThresholdInputs<'_>) -> bool {
    input.candidates.len() < 40
        || input.candidate_repositories < 5
        || input.max_repository_share > 0.25
}

fn quiet_distribution_failed(input: &ThresholdInputs<'_>) -> bool {
    input.quiet.len() < 20 || input.quiet_repositories < 3
}

fn dimension_lower_bound_failed(
    input: &ThresholdInputs<'_>,
    dimension: &str,
    minimum: f64,
) -> bool {
    input.dimensions[dimension].positive_rate.wilson_95_lower < minimum
}

fn reviewer_agreement_failed(input: &ThresholdInputs<'_>) -> bool {
    input
        .dimensions
        .values()
        .any(|value| value.raw_agreement < 0.80 || value.cohens_kappa < 0.60)
}

fn fixture_recall_failed(input: &ThresholdInputs<'_>) -> bool {
    input.fixture.total == 0 || input.fixture.successes as f64 / (input.fixture.total as f64) < 0.90
}

fn unsupported_coverage_failed(input: &ThresholdInputs<'_>) -> bool {
    input.unsupported.total == 0 || input.unsupported.successes != input.unsupported.total
}

fn final_judgment(input: &GroupInputs<'_>, site: &PacketSite, dimension: &str) -> bool {
    input
        .adjudicated
        .map(|labels| labels[site.id.as_str()].judgments[dimension])
        .unwrap_or(
            input.reviewer_a[site.id.as_str()].judgments[dimension]
                && input.reviewer_b[site.id.as_str()].judgments[dimension],
        )
}

fn fixture_rate(input: &GroupInputs<'_>) -> Rate {
    let fixtures = input
        .sites
        .iter()
        .filter_map(|site| site.fixture_expected.map(|expected| (*site, expected)))
        .collect::<Vec<_>>();
    rate(
        fixtures
            .iter()
            .filter(|(site, expected)| {
                final_judgment(input, site, "detection_claim_correctness") == *expected
            })
            .count(),
        fixtures.len(),
    )
}

fn unsupported_rate(input: &GroupInputs<'_>) -> Rate {
    let unsupported = input
        .sites
        .iter()
        .filter(|site| site.unsupported_expected)
        .collect::<Vec<_>>();
    rate(
        unsupported
            .iter()
            .filter(|site| final_judgment(input, site, "coverage_honesty"))
            .count(),
        unsupported.len(),
    )
}

fn repository_count(sites: &[&PacketSite]) -> usize {
    sites
        .iter()
        .map(|site| site.repository_bucket.as_str())
        .collect::<BTreeSet<_>>()
        .len()
}

fn max_repository_share(sites: &[&PacketSite]) -> f64 {
    if sites.is_empty() {
        return 0.0;
    }
    let mut counts = BTreeMap::new();
    for site in sites {
        *counts
            .entry(site.repository_bucket.as_str())
            .or_insert(0usize) += 1;
    }
    counts.values().copied().max().unwrap_or_default() as f64 / sites.len() as f64
}

pub(crate) fn agreement(pairs: &[(bool, bool, bool)]) -> f64 {
    if pairs.is_empty() {
        return 0.0;
    }
    pairs.iter().filter(|(a, b, _)| a == b).count() as f64 / pairs.len() as f64
}

pub(crate) fn kappa(pairs: &[(bool, bool, bool)]) -> f64 {
    if pairs.is_empty() {
        return 0.0;
    }
    let count = pairs.len() as f64;
    let observed = agreement(pairs);
    let a_yes = pairs.iter().filter(|(a, _, _)| *a).count() as f64 / count;
    let b_yes = pairs.iter().filter(|(_, b, _)| *b).count() as f64 / count;
    let expected = a_yes * b_yes + (1.0 - a_yes) * (1.0 - b_yes);
    if (1.0 - expected).abs() < f64::EPSILON {
        return if observed == 1.0 { 1.0 } else { 0.0 };
    }
    (observed - expected) / (1.0 - expected)
}

fn rate(successes: usize, total: usize) -> Rate {
    Rate {
        successes,
        total,
        wilson_95_lower: wilson_lower(successes, total),
    }
}

pub(crate) fn wilson_lower(successes: usize, total: usize) -> f64 {
    if total == 0 {
        return 0.0;
    }
    let z = 1.959_963_984_540_054;
    let n = total as f64;
    let p = successes as f64 / n;
    let denominator = 1.0 + z * z / n;
    let center = p + z * z / (2.0 * n);
    let margin = z * ((p * (1.0 - p) + z * z / (4.0 * n)) / n).sqrt();
    (center - margin) / denominator
}

fn labels_by_site(labels: &LabelFile) -> BTreeMap<&str, &SiteLabel> {
    labels
        .labels
        .iter()
        .map(|label| (label.site_id.as_str(), label))
        .collect()
}
