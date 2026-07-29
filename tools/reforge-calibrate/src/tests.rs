use super::*;
use crate::model::{PacketSite, ReviewPacket};
use crate::packet::{packet_digest, site_id};

fn packet_site(repository_bucket: &str) -> PacketSite {
    let summary = serde_json::json!({"path": "src/lib.rs", "line": 10});
    PacketSite {
        id: site_id(
            "reforge.codebase.large_file",
            "rust",
            repository_bucket,
            &summary,
        )
        .unwrap(),
        rule: "reforge.codebase.large_file".into(),
        language: "rust".into(),
        repository_bucket: repository_bucket.into(),
        candidate: true,
        quiet: false,
        fixture_expected: Some(true),
        unsupported_expected: false,
        summary,
    }
}

fn packet(sites: Vec<PacketSite>) -> ReviewPacket {
    ReviewPacket {
        packet_schema_version: 1,
        packet_digest: packet_digest(&sites).unwrap(),
        sites,
    }
}

#[test]
fn wilson_lower_bound_is_conservative() {
    assert!(statistics::wilson_lower(40, 40) > 0.91);
    assert!(statistics::wilson_lower(36, 40) < 0.80);
    assert_eq!(statistics::wilson_lower(0, 0), 0.0);
}

#[test]
fn agreement_and_kappa_are_independent() {
    let pairs = [
        (true, true, true),
        (false, false, false),
        (true, false, false),
    ];
    assert_eq!(statistics::agreement(&pairs), 2.0 / 3.0);
    assert!(statistics::kappa(&pairs) < statistics::agreement(&pairs));
}

#[test]
fn packet_digest_detects_site_mutation() {
    let mut packet = packet(vec![packet_site("repository-a")]);
    packet.sites[0].candidate = false;
    assert!(validation::validate_packet(&packet).is_err());
}

#[test]
fn packet_rejects_duplicate_and_overlapping_sites() {
    let duplicate = packet(vec![
        packet_site("repository-a"),
        packet_site("repository-a"),
    ]);
    assert!(validation::validate_packet(&duplicate).is_err());

    let mut overlapping_site = packet_site("repository-a");
    overlapping_site.quiet = true;
    let overlapping = packet(vec![overlapping_site]);
    assert!(validation::validate_packet(&overlapping).is_err());
}
