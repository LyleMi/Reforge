use std::path::Path;

use anyhow::{Result, bail};

use crate::io::{read_json, short_digest, write_json};
use crate::model::{CandidateSite, PacketSite, ReviewPacket};
use crate::validation::validate_packet;

pub(crate) fn generate_packet(input: &Path, output: &Path) -> Result<()> {
    let candidates = read_json::<Vec<CandidateSite>>(input)?;
    let mut sites = candidates
        .into_iter()
        .map(packet_site)
        .collect::<Result<Vec<_>>>()?;
    sites.sort_by(|left, right| left.id.cmp(&right.id));
    let packet = ReviewPacket {
        packet_schema_version: 1,
        packet_digest: packet_digest(&sites)?,
        sites,
    };
    validate_packet(&packet)?;
    write_json(output, &packet)
}

fn packet_site(site: CandidateSite) -> Result<PacketSite> {
    if site.rule.trim().is_empty()
        || site.language.trim().is_empty()
        || site.repository.trim().is_empty()
    {
        bail!("candidate rule, language, and repository must not be empty");
    }
    if site.candidate && site.quiet {
        bail!("a calibration site cannot be both candidate and quiet");
    }
    let repository_bucket = short_digest("repository", &site.repository);
    Ok(PacketSite {
        id: site_id(
            &site.rule,
            &site.language,
            &repository_bucket,
            &site.summary,
        )?,
        rule: site.rule,
        language: site.language,
        repository_bucket,
        candidate: site.candidate,
        quiet: site.quiet,
        fixture_expected: site.fixture_expected,
        unsupported_expected: site.unsupported_expected,
        summary: site.summary,
    })
}

pub(crate) fn packet_digest(sites: &[PacketSite]) -> Result<String> {
    Ok(short_digest("packet", &serde_json::to_string(sites)?))
}

pub(crate) fn site_id(
    rule: &str,
    language: &str,
    repository_bucket: &str,
    summary: &serde_json::Value,
) -> Result<String> {
    let semantic = serde_json::to_vec(&(rule, language, repository_bucket, summary))?;
    Ok(short_digest("site", &String::from_utf8_lossy(&semantic)))
}
