use super::*;

fn source_file(path: &str, source: &str) -> SourceFile {
    SourceFile {
        path: PathBuf::from(path),
        display_path: path.to_string(),
        source: source.into(),
    }
}

fn options() -> SimilarFunctionOptions {
    SimilarFunctionOptions {
        min_group_size: 3,
        min_tokens: 12,
        threshold: 0.80,
        include_test_similarity: false,
    }
}

fn metric_value(finding: &Finding, name: &str) -> Option<usize> {
    finding
        .metrics
        .iter()
        .find(|metric| metric.name == name)
        .map(|metric| metric.value)
}

#[test]
fn detects_similar_rust_functions() -> Result<()> {
    let source = r#"
fn alpha(items: &[i32]) -> i32 {
    let mut total = 0;
    for item in items {
        if *item > 10 {
            total += *item * 2;
        } else {
            total += *item;
        }
    }
    total
}

fn beta(values: &[i32]) -> i32 {
    let mut sum = 0;
    for value in values {
        if *value > 20 {
            sum += *value * 2;
        } else {
            sum += *value;
        }
    }
    sum
}

fn gamma(numbers: &[i32]) -> i32 {
    let mut acc = 0;
    for number in numbers {
        if *number > 30 {
            acc += *number * 2;
        } else {
            acc += *number;
        }
    }
    acc
}
"#;

    let findings = scan_similar_functions(&[source_file("src/lib.rs", source)], &options())?;

    assert_eq!(findings.len(), 1);
    assert_eq!(metric_value(&findings[0], "group_size"), Some(3));
    Ok(())
}

#[test]
fn skips_rust_cfg_test_modules_for_similarity_by_default() -> Result<()> {
    let source = r#"
fn production() -> i32 {
    1
}

#[cfg(test)]
mod tests {
    fn alpha(items: &[i32]) -> i32 {
        let mut total = 0;
        for item in items {
            if *item > 10 {
                total += *item * 2;
            } else {
                total += *item;
            }
        }
        total
    }

    fn beta(values: &[i32]) -> i32 {
        let mut sum = 0;
        for value in values {
            if *value > 20 {
                sum += *value * 2;
            } else {
                sum += *value;
            }
        }
        sum
    }

    fn gamma(numbers: &[i32]) -> i32 {
        let mut acc = 0;
        for number in numbers {
            if *number > 30 {
                acc += *number * 2;
            } else {
                acc += *number;
            }
        }
        acc
    }
}
"#;

    let scan = scan_similar_functions_report(&[source_file("src/lib.rs", source)], &options())?;

    assert_eq!(scan.candidate_count, 0);
    assert!(scan.findings.is_empty());

    let mut opts = options();
    opts.include_test_similarity = true;
    let included = scan_similar_functions_report(&[source_file("src/lib.rs", source)], &opts)?;

    assert_eq!(included.candidate_count, 3);
    assert_eq!(included.findings.len(), 1);
    assert_eq!(metric_value(&included.findings[0], "group_size"), Some(3));
    Ok(())
}

#[test]
fn ignores_short_trivial_functions() -> Result<()> {
    let source = "fn a() { 1 }\nfn b() { 2 }\nfn c() { 3 }\n";

    let findings = scan_similar_functions(&[source_file("src/lib.rs", source)], &options())?;

    assert!(findings.is_empty());
    Ok(())
}

#[test]
fn ignores_anonymous_javascript_callbacks() -> Result<()> {
    let source = r#"
items.map(function (item) {
  const total = item.value + 10;
  if (total > 20) {
    return total * 2;
  }
  return total;
});
items.map((entry) => {
  const sum = entry.value + 10;
  if (sum > 20) {
    return sum * 2;
  }
  return sum;
});
"#;

    let findings = scan_similar_functions(&[source_file("src/app.js", source)], &options())?;

    assert!(findings.is_empty());
    Ok(())
}

#[test]
fn detects_similar_javascript_functions_with_normalized_names_and_literals() -> Result<()> {
    let source = javascript_three_similar_functions();

    let findings = scan_similar_functions(&[source_file("src/app.js", source)], &options())?;

    assert_eq!(findings.len(), 1);
    assert_eq!(metric_value(&findings[0], "group_size"), Some(3));
    assert_eq!(findings[0].related_locations.len(), 3);
    Ok(())
}

#[test]
fn detects_similar_typescript_functions() -> Result<()> {
    let source = r#"
function alpha(items: Item[]): number {
  let total = 0;
  for (const item of items) {
    if (item.score > 10) {
      total += item.score * 2;
    } else {
      total += item.score;
    }
  }
  return total;
}

function beta(records: Item[]): number {
  let sum = 1;
  for (const record of records) {
    if (record.score > 20) {
      sum += record.score * 2;
    } else {
      sum += record.score;
    }
  }
  return sum;
}

function gamma(rows: Item[]): number {
  let acc = 2;
  for (const row of rows) {
    if (row.score > 30) {
      acc += row.score * 2;
    } else {
      acc += row.score;
    }
  }
  return acc;
}
"#;

    let findings = scan_similar_functions(&[source_file("src/app.ts", source)], &options())?;

    assert_eq!(findings.len(), 1);
    assert_eq!(metric_value(&findings[0], "group_size"), Some(3));
    Ok(())
}

#[test]
fn detects_similar_python_functions() -> Result<()> {
    let source = r#"
def alpha(items):
    total = 0
    for item in items:
        if item.score > 10:
            total += item.score * 2
        else:
            total += item.score
    return total

async def beta(records):
    sum_value = 1
    for record in records:
        if record.score > 20:
            sum_value += record.score * 2
        else:
            sum_value += record.score
    return sum_value

def gamma(rows):
    acc = 2
    for row in rows:
        if row.score > 30:
            acc += row.score * 2
        else:
            acc += row.score
    return acc
"#;

    let findings = scan_similar_functions(&[source_file("src/app.py", source)], &options())?;

    assert_eq!(findings.len(), 1);
    Ok(())
}

#[test]
fn detects_similar_go_functions() -> Result<()> {
    let source = r#"
package app

func Alpha(items []Item) int {
    total := 0
    for _, item := range items {
        if item.Score > 10 {
            total += item.Score * 2
        } else {
            total += item.Score
        }
    }
    return total
}

func Beta(records []Item) int {
    sum := 1
    for _, record := range records {
        if record.Score > 20 {
            sum += record.Score * 2
        } else {
            sum += record.Score
        }
    }
    return sum
}

func Gamma(rows []Item) int {
    acc := 2
    for _, row := range rows {
        if row.Score > 30 {
            acc += row.Score * 2
        } else {
            acc += row.Score
        }
    }
    return acc
}
"#;

    let findings = scan_similar_functions(&[source_file("src/app.go", source)], &options())?;

    assert_eq!(findings.len(), 1);
    Ok(())
}

#[test]
fn requires_minimum_group_size() -> Result<()> {
    let source = r#"
function alpha(items) {
  let total = 0;
  for (const item of items) {
    total += item.score;
  }
  return total;
}

function beta(records) {
  let sum = 1;
  for (const record of records) {
    sum += record.score;
  }
  return sum;
}
"#;

    let findings = scan_similar_functions(&[source_file("src/app.js", source)], &options())?;

    assert!(findings.is_empty());
    Ok(())
}

#[test]
fn same_names_with_different_structure_are_not_grouped() -> Result<()> {
    let files = vec![
        source_file(
            "src/a.js",
            r#"
function process(items) {
  let total = 0;
  for (const item of items) {
    total += item.score;
  }
  return total;
}
"#,
        ),
        source_file(
            "src/b.js",
            r#"
function process(items) {
  const names = [];
  for (const item of items) {
    names.push(item.name.toUpperCase());
  }
  return names.join(",");
}
"#,
        ),
        source_file(
            "src/c.js",
            r#"
function process(items) {
  const map = new Map();
  for (const item of items) {
    map.set(item.id, item);
  }
  return map;
}
"#,
        ),
    ];

    let mut strict_options = options();
    strict_options.threshold = 0.95;
    let findings = scan_similar_functions(&files, &strict_options)?;

    assert!(findings.is_empty());
    Ok(())
}

#[test]
fn custom_threshold_changes_detection() -> Result<()> {
    let source = r#"
function alpha(items) {
  let total = 0;
  for (const item of items) {
    if (item.score > 10) total += item.score * 2;
  }
  return total;
}
function beta(items) {
  let total = 0;
  for (const item of items) {
    if (item.score > 10) total += item.score * 2;
  }
  return total;
}
function gamma(items) {
  let total = 0;
  for (const item of items) {
    while (item.active) {
      total += item.score * 2;
      break;
    }
  }
  return total;
}
"#;

    let mut relaxed = options();
    relaxed.threshold = 0.60;
    let mut strict = options();
    strict.threshold = 0.95;

    assert_eq!(
        scan_similar_functions(&[source_file("src/app.js", source)], &relaxed)?.len(),
        1
    );
    assert!(scan_similar_functions(&[source_file("src/app.js", source)], &strict)?.is_empty());
    Ok(())
}

#[test]
fn reports_candidate_count() -> Result<()> {
    let scan = scan_similar_functions_report(
        &[source_file(
            "src/app.js",
            javascript_three_similar_functions(),
        )],
        &options(),
    )?;

    assert_eq!(scan.candidate_count, 3);
    assert_eq!(scan.findings.len(), 1);
    Ok(())
}

#[test]
fn length_ratio_pruning_keeps_matching_group() -> Result<()> {
    let source = javascript_three_similar_functions();

    let findings = scan_similar_functions(&[source_file("src/app.js", source)], &options())?;

    assert_eq!(findings.len(), 1);
    Ok(())
}

#[test]
fn multiset_pruning_keeps_matching_group() -> Result<()> {
    let source = r#"
function alpha(items) {
  let total = 0;
  for (const item of items) {
    total += item.score;
    total += item.weight;
  }
  return total;
}
function beta(records) {
  let sum = 1;
  for (const record of records) {
    sum += record.score;
    sum += record.weight;
  }
  return sum;
}
function gamma(rows) {
  let acc = 2;
  for (const row of rows) {
    acc += row.score;
    acc += row.weight;
  }
  return acc;
}
"#;

    let findings = scan_similar_functions(&[source_file("src/app.js", source)], &options())?;

    assert_eq!(findings.len(), 1);
    Ok(())
}

fn javascript_three_similar_functions() -> &'static str {
    r#"
function alpha(items) {
  let total = 0;
  for (const item of items) {
    if (item.score > 10) {
      total += item.score * 2;
    } else {
      total += item.score;
    }
  }
  return total;
}

function beta(records) {
  let sum = 1;
  for (const record of records) {
    if (record.score > 20) {
      sum += record.score * 2;
    } else {
      sum += record.score;
    }
  }
  return sum;
}

function gamma(rows) {
  let acc = 2;
  for (const row of rows) {
    if (row.score > 30) {
      acc += row.score * 2;
    } else {
      acc += row.score;
    }
  }
  return acc;
}
"#
}
