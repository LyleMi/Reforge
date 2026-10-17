use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::detectors::similarity::SourceFile;
use crate::evidence_analysis::DetectedEvidenceInput;
use crate::model::{DetectedEvidence, DetectedMeasurement, MetricId, RelatedLocation, Rule};

/// Hash first, then compare full decoded contents. No source-pair matrix or
/// syntax parsing is required, including for large generated case snapshots.
pub(crate) fn scan(files: &[SourceFile], min_lines: usize) -> Vec<DetectedEvidence> {
    let mut buckets: BTreeMap<(usize, [u8; 32]), Vec<&SourceFile>> = BTreeMap::new();
    for file in files {
        if file.source.lines().count() < min_lines || file.source.trim().is_empty() {
            continue;
        }
        buckets
            .entry((
                file.source.len(),
                Sha256::digest(file.source.as_bytes()).into(),
            ))
            .or_default()
            .push(file);
    }
    let mut detections = Vec::new();
    for bucket in buckets.values_mut().filter(|bucket| bucket.len() > 1) {
        bucket.sort_by(|left, right| left.display_path.cmp(&right.display_path));
        // Exact comparison makes the grouping independent of hash collisions.
        let mut groups: Vec<Vec<&SourceFile>> = Vec::new();
        for &file in bucket.iter() {
            if let Some(group) = groups
                .iter_mut()
                .find(|group| group[0].source == file.source)
            {
                group.push(file);
            } else {
                groups.push(vec![file]);
            }
        }
        for group in groups.into_iter().filter(|group| group.len() > 1) {
            detections.push(detection(&group, min_lines));
        }
    }
    detections
}

fn detection(group: &[&SourceFile], min_lines: usize) -> DetectedEvidence {
    let first = group[0];
    DetectedEvidenceInput::new(
        Rule::DuplicateFile, &first.display_path, Some(1),
        format!("{} files contain identical decoded source ({} bytes each); review shared ownership or document intentional template/snapshot copies", group.len(), first.source.len()),
        vec![
            DetectedMeasurement::measurement(MetricId::GroupSize, group.len(), "files"),
            DetectedMeasurement::threshold(MetricId::FileLoc, first.source.lines().count(), min_lines, "lines"),
        ],
    ).with_group_subject().with_related_locations(group.iter().map(|file| RelatedLocation {
        path: file.display_path.clone(), line: 1, name: None, entity_key: None,
    }).collect()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, source: &str) -> SourceFile {
        SourceFile {
            path: path.into(),
            display_path: path.into(),
            source: source.into(),
        }
    }

    #[test]
    fn groups_complete_copies_with_all_paths_and_stable_order() {
        let sources = [
            file("cases/a.py", "value = 1\n"),
            file("recipe/a.py", "value = 1\n"),
            file("cases/b.py", "value = 2\n"),
        ];
        let first = scan(&sources, 1);
        let mut reversed = sources.to_vec();
        reversed.reverse();
        assert_eq!(first, scan(&reversed, 1));
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].related_locations.len(), 2);
        assert!(scan(&sources, 2).is_empty());
        assert!(scan(&[file("a.py", "\n"), file("b.py", "\n")], 1).is_empty());
    }
}
