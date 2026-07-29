use crate::detectors::manifest::input_metrics;
use crate::model::{DetectedEvidence, DetectedMeasurement, DetectedSubject, RelatedLocation, Rule};

#[derive(Debug, Clone)]
pub struct DetectedEvidenceInput {
    kind: Rule,
    subject: Option<DetectedSubject>,
    language: Option<String>,
    path: String,
    line: Option<usize>,
    message: String,
    metrics: Vec<DetectedMeasurement>,
    related_locations: Vec<RelatedLocation>,
}

impl DetectedEvidenceInput {
    pub fn new(
        kind: Rule,
        path: impl Into<String>,
        line: Option<usize>,
        message: impl Into<String>,
        metrics: Vec<DetectedMeasurement>,
    ) -> Self {
        let declared_metrics = input_metrics(kind);
        assert!(
            metrics
                .iter()
                .all(|metric| declared_metrics.contains(&metric.name)),
            "detection {kind:?} emitted a metric outside its detector contract"
        );
        Self {
            kind,
            subject: None,
            language: None,
            path: path.into(),
            line,
            message: message.into(),
            metrics,
            related_locations: Vec::new(),
        }
    }

    pub fn with_related_locations(mut self, related_locations: Vec<RelatedLocation>) -> Self {
        self.related_locations = related_locations;
        self
    }

    pub fn with_subject(mut self, subject: DetectedSubject, language: impl Into<String>) -> Self {
        self.subject = Some(subject);
        self.language = Some(language.into());
        self
    }

    pub fn with_file_subject(self) -> Self {
        let language = language_for_path(&self.path);
        self.with_subject(DetectedSubject::File, language)
    }

    pub fn with_directory_subject(self) -> Self {
        self.with_subject(DetectedSubject::Directory, "language_neutral_paths")
    }

    pub fn with_group_subject(self) -> Self {
        let language = language_for_path(&self.path);
        self.with_subject(DetectedSubject::Group, language)
    }

    pub fn with_symbol_subject(
        self,
        declaration_kind: impl Into<String>,
        name: impl Into<String>,
        signature: Option<String>,
    ) -> Self {
        let language = language_for_path(&self.path);
        self.with_subject(
            DetectedSubject::Symbol {
                declaration_kind: declaration_kind.into(),
                name: name.into(),
                signature,
            },
            language,
        )
    }
}

fn language_for_path(path: &str) -> &'static str {
    match std::path::Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some("rs") => "rust",
        Some("js" | "jsx" | "mjs" | "cjs") => "javascript",
        Some("ts" | "mts" | "cts") => "typescript",
        Some("tsx" | "vue") => "tsx",
        Some("py") => "python",
        Some("go") => "go",
        Some("java") => "java",
        Some("cs" | "csx") => "csharp",
        Some("kt") => "kotlin",
        Some("php") => "php",
        Some("rb") => "ruby",
        Some("sh" | "bash") => "bash",
        Some("ps1" | "psm1") => "powershell",
        Some("c") => "c",
        Some("cc" | "cpp") => "cpp",
        _ => "unknown",
    }
}

impl From<DetectedEvidenceInput> for DetectedEvidence {
    fn from(input: DetectedEvidenceInput) -> Self {
        let subject = input
            .subject
            .expect("detectors must provide a typed subject");
        let subject_kind = match &subject {
            DetectedSubject::Repository => crate::model::SubjectKind::Repository,
            DetectedSubject::Directory => crate::model::SubjectKind::Directory,
            DetectedSubject::File => crate::model::SubjectKind::File,
            DetectedSubject::Symbol { .. } => crate::model::SubjectKind::Symbol,
            DetectedSubject::Group => crate::model::SubjectKind::Group,
        };
        let contract = crate::detectors::manifest::rule_registry()
            .iter()
            .find(|entry| entry.kind == input.kind)
            .expect("detected rule must be registered");
        assert!(
            contract.allowed_subjects.contains(&subject_kind),
            "detection {:?} emitted a subject outside its detector contract",
            input.kind
        );
        DetectedEvidence {
            semantic_anchor: format!("path:{}", crate::pathing::normalize_path_text(&input.path)),
            kind: input.kind,
            subject,
            language: input.language.expect("detectors must provide a language"),
            path: input.path,
            line: input.line,
            metrics: input.metrics,
            message: input.message,
            related_locations: input.related_locations,
            flow_witness: None,
        }
    }
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    use crate::model::MetricId;

    #[test]
    #[should_panic(expected = "outside its detector contract")]
    fn rejects_metrics_not_declared_by_detector() {
        let _ = DetectedEvidence::from(
            DetectedEvidenceInput::new(
                Rule::LargeFile,
                "src/lib.rs",
                Some(1),
                "",
                vec![DetectedMeasurement::threshold(
                    MetricId::GroupSize,
                    3,
                    2,
                    "items",
                )],
            )
            .with_file_subject(),
        );
    }
}
