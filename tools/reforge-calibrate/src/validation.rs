use std::collections::BTreeSet;

use anyhow::{Result, bail};

use crate::model::{LabelFile, ReviewPacket};
use crate::packet::{packet_digest, site_id};

pub(crate) const DIMENSIONS: [&str; 7] = [
    "instrumentation_correctness",
    "detection_claim_correctness",
    "useful_for_inspection",
    "legitimate_exception",
    "suggested_action_suitability",
    "clustering_correctness",
    "coverage_honesty",
];

pub(crate) fn validate_labels(packet: &ReviewPacket, labels: &LabelFile) -> Result<()> {
    validate_packet(packet)?;
    validate_label_header(packet, labels)?;
    validate_label_sites(packet, labels)?;
    Ok(())
}

fn validate_label_header(packet: &ReviewPacket, labels: &LabelFile) -> Result<()> {
    if labels.label_schema_version != 1 {
        bail!("unsupported calibration packet or label schema");
    }
    if packet.packet_digest != labels.packet_digest {
        bail!("labels were produced for a different review packet");
    }
    for value in [
        &labels.reviewer.reviewer_type,
        &labels.reviewer.model,
        &labels.reviewer.version,
        &labels.reviewer.prompt_digest,
        &labels.reviewer.timestamp,
    ] {
        if value.trim().is_empty() {
            bail!("reviewer provenance fields must not be empty");
        }
    }
    Ok(())
}

fn validate_label_sites(packet: &ReviewPacket, labels: &LabelFile) -> Result<()> {
    let expected = packet
        .sites
        .iter()
        .map(|site| site.id.as_str())
        .collect::<BTreeSet<_>>();
    let actual = labels
        .labels
        .iter()
        .map(|label| label.site_id.as_str())
        .collect::<BTreeSet<_>>();
    if expected != actual || actual.len() != labels.labels.len() {
        bail!("labels must cover every packet site exactly once");
    }
    for label in &labels.labels {
        let dimensions = label
            .judgments
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if dimensions != BTreeSet::from(DIMENSIONS) {
            bail!(
                "site {} does not contain the seven required judgments",
                label.site_id
            );
        }
    }
    Ok(())
}

pub(crate) fn validate_packet(packet: &ReviewPacket) -> Result<()> {
    if packet.packet_schema_version != 1 {
        bail!("unsupported calibration packet schema");
    }
    if packet.packet_digest != packet_digest(&packet.sites)? {
        bail!("calibration packet digest does not match its sites");
    }
    let mut ids = BTreeSet::new();
    for site in &packet.sites {
        validate_packet_site(site)?;
        if !ids.insert(site.id.as_str()) {
            bail!("packet contains duplicate site {}", site.id);
        }
    }
    Ok(())
}

fn validate_packet_site(site: &crate::model::PacketSite) -> Result<()> {
    if [
        site.rule.as_str(),
        site.language.as_str(),
        site.repository_bucket.as_str(),
    ]
    .iter()
    .any(|value| value.trim().is_empty())
    {
        bail!("packet site rule, language, and repository bucket must not be empty");
    }
    if site.candidate && site.quiet {
        bail!("a calibration site cannot be both candidate and quiet");
    }
    let expected_id = site_id(
        &site.rule,
        &site.language,
        &site.repository_bucket,
        &site.summary,
    )?;
    if site.id != expected_id {
        bail!("packet site {} has an invalid stable ID", site.id);
    }
    Ok(())
}
