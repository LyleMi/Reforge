use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseFailureReason {
    SyntaxError,
    ParserFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFailureReason {
    IoError,
    UnsupportedEncoding,
    InvalidEncoding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceFailure {
    pub path: String,
    pub reason: SourceFailureReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseFailure {
    pub path: String,
    pub language: String,
    pub reason: ParseFailureReason,
}

pub(crate) fn source_failure_limitations<'a>(
    failures: impl Iterator<Item = &'a SourceFailure>,
) -> Vec<reforge_schema::CoverageLimitation> {
    let count = failures.count();
    if count == 0 {
        return Vec::new();
    }
    vec![reforge_schema::CoverageLimitation {
        code: "source_read_failure".into(),
        count,
        message: "source files could not be read".into(),
    }]
}
