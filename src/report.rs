use crate::scanner::{Finding, Severity};

pub fn print_findings(findings: &[Finding]) {
    if findings.is_empty() {
        println!("No refactoring signals found.");
        return;
    }

    for finding in findings {
        println!(
            "[{}] {}:{} {}",
            finding.severity, finding.path, finding.line, finding.message
        );
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info => write!(f, "info"),
            Severity::Warning => write!(f, "warning"),
        }
    }
}
