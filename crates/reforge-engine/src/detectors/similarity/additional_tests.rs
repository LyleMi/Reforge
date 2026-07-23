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

    let detections = scan_similar_functions(&[source_file("src/app.js", source)], &options())?;

    assert!(detections.is_empty());
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
    let detections = scan_similar_functions(&files, &strict_options)?;

    assert!(detections.is_empty());
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
    assert_eq!(scan.detections.len(), 1);
    Ok(())
}

#[test]
fn length_ratio_pruning_keeps_matching_group() -> Result<()> {
    let source = javascript_three_similar_functions();

    let detections = scan_similar_functions(&[source_file("src/app.js", source)], &options())?;

    assert_eq!(detections.len(), 1);
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

    let detections = scan_similar_functions(&[source_file("src/app.js", source)], &options())?;

    assert_eq!(detections.len(), 1);
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
