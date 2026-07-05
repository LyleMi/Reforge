use crate::scanner::{Finding, Severity};

pub fn print_findings(findings: &[Finding]) {
    if findings.is_empty() {
        println!("No refactoring signals found.");
        return;
    }

    for finding in sorted_findings(findings) {
        println!(
            "[{}] {}:{} {}",
            finding.severity, finding.path, finding.line, finding.message
        );
    }
}

fn sorted_findings(findings: &[Finding]) -> Vec<&Finding> {
    let mut sorted: Vec<&Finding> = findings.iter().collect();

    sorted.sort_by(|left, right| match (left.total_lines, right.total_lines) {
        (Some(left_lines), Some(right_lines)) => right_lines
            .cmp(&left_lines)
            .then_with(|| left.path.cmp(&right.path)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });

    sorted
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info => write!(f, "info"),
            Severity::Warning => write!(f, "warning"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(path: &str, total_lines: Option<usize>) -> Finding {
        Finding {
            severity: if total_lines.is_some() {
                Severity::Warning
            } else {
                Severity::Info
            },
            path: path.to_string(),
            line: 1,
            total_lines,
            message: String::new(),
        }
    }

    #[test]
    fn sorts_large_files_by_total_lines_descending() {
        let findings = vec![
            finding("src/small_todo.rs", None),
            finding("src/large.rs", Some(900)),
            finding("src/largest.rs", Some(1_200)),
            finding("src/medium.rs", Some(1_000)),
            finding("src/another_todo.rs", None),
        ];

        let paths: Vec<&str> = sorted_findings(&findings)
            .iter()
            .map(|finding| finding.path.as_str())
            .collect();

        assert_eq!(
            paths,
            vec![
                "src/largest.rs",
                "src/medium.rs",
                "src/large.rs",
                "src/small_todo.rs",
                "src/another_todo.rs"
            ]
        );
    }
}
