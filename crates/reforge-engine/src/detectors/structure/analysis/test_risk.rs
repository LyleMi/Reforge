pub(super) fn collect_happy_path_test_risk(
    file: &SourceFile,
    family: LanguageFamily,
    signals: &mut FileSignals,
) {
    let test_cases = test_case_occurrences(file, family);
    if test_cases.len() < 3 {
        return;
    }

    if has_assertion_evidence(&file.source) && !has_negative_or_boundary_test_evidence(&file.source)
    {
        signals
            .happy_path_test_files
            .push((test_cases.len(), test_cases));
    }
}

pub(super) fn happy_path_test_detections(test_files: Vec<(usize, Vec<Occurrence>)>) -> Vec<DetectedEvidence> {
    test_files
        .into_iter()
        .filter_map(|(test_count, locations)| {
            let representative = locations.first()?;
            Some(crate::model::DetectedEvidence::from(
                DetectedEvidenceInput::new(
                    Rule::HappyPathOnlyTests,
                    representative.path.clone(),
                    Some(representative.line),
                    format!(
                        "test file has {test_count} cases but no negative, error, or boundary assertions were detected"
                    ),
                    vec![DetectedMeasurement::threshold(
                        crate::model::MetricId::GroupSize,
                        test_count,
                        3,
                        "test cases",
                    )],
                )
                .with_related_locations(locations),
            ))
        })
        .collect()
}

pub(super) fn test_case_occurrences(file: &SourceFile, family: LanguageFamily) -> Vec<Occurrence> {
    file.source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim_start();
            if is_test_case_line(trimmed, family) {
                Some(Occurrence {
                    path: file.display_path.clone(),
                    line: index + 1,
                    name: test_case_name(trimmed, family),
                })
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn is_test_case_line(line: &str, family: LanguageFamily) -> bool {
    match family {
        LanguageFamily::Rust => {
            line.starts_with("#[test]")
                || line.starts_with("#[tokio::test")
                || line.starts_with("#[async_std::test")
        }
        LanguageFamily::JavaScriptTypeScript => {
            line.starts_with("test(")
                || line.starts_with("it(")
                || line.starts_with("test.each")
                || line.starts_with("it.each")
        }
        LanguageFamily::Python => {
            line.starts_with("def test_") || line.starts_with("async def test_")
        }
        LanguageFamily::Go => line.starts_with("func Test"),
        LanguageFamily::Java | LanguageFamily::CSharp | LanguageFamily::Kotlin => {
            line.starts_with("@Test")
                || line.starts_with("[Test")
                || line.starts_with("[Fact")
                || line.starts_with("[Theory")
        }
        LanguageFamily::Php => {
            line.starts_with("public function test") || line.starts_with("function test")
        }
        LanguageFamily::Ruby => line.starts_with("def test_") || line.starts_with("it "),
        LanguageFamily::Bash | LanguageFamily::PowerShell => false,
    }
}

pub(super) fn test_case_name(line: &str, family: LanguageFamily) -> Option<String> {
    match family {
        LanguageFamily::Rust => Some("test attribute".to_string()),
        LanguageFamily::JavaScriptTypeScript => quoted_test_name(line),
        LanguageFamily::Python
        | LanguageFamily::Go
        | LanguageFamily::Java
        | LanguageFamily::CSharp
        | LanguageFamily::Kotlin
        | LanguageFamily::Php
        | LanguageFamily::Ruby
        | LanguageFamily::Bash
        | LanguageFamily::PowerShell => line
            .split(['(', '{'])
            .next()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(ToString::to_string),
    }
}

pub(super) fn quoted_test_name(line: &str) -> Option<String> {
    let quote_index = line.find(['"', '\'', '`'])?;
    let quote = line[quote_index..].chars().next()?;
    let rest = &line[quote_index + quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

pub(super) fn has_assertion_evidence(source: &str) -> bool {
    let normalized = source.to_ascii_lowercase();
    [
        "expect(", "assert", "should", "t.error", "t.fatal", "require.", "pytest.",
    ]
    .iter()
    .any(|pattern| normalized.contains(pattern))
}

pub(super) fn has_negative_or_boundary_test_evidence(source: &str) -> bool {
    let normalized = source.to_ascii_lowercase();
    [
        "tothrow",
        "to_throw",
        ".rejects",
        "raises(",
        "pytest.raises",
        "should_panic",
        "is_err",
        "unwrap_err",
        "expect_err",
        " err == nil",
        "err == nil",
        " err != nil",
        "err != nil",
        "invalid",
        "missing",
        "empty",
        "none",
        "null",
        "nil",
        "zero",
        "negative",
        "unauthorized",
        "forbidden",
        "not found",
        "not_found",
        "error",
        "failure",
        "panic",
        "duplicate",
        "overflow",
        "underflow",
        "timeout",
        "denied",
        "boundary",
        "edge",
        "does not",
        "doesn't",
        "without",
        "ignore",
        "ignores",
        "ignored",
        "skip",
        "skips",
        "skipped",
        "caps",
        "prevents",
    ]
    .iter()
    .any(|pattern| normalized.contains(pattern))
}
