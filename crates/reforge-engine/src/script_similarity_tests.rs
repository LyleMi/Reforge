use super::*;
use std::path::PathBuf;

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

#[test]
fn detects_similar_bash_functions() -> Result<()> {
    let source = r#"
alpha() {
  total=0
  for item in "$@"; do
    if [ "$item" -gt 10 ]; then
      total=$((total + item * 2))
    else
      total=$((total + item))
    fi
  done
  echo "$total"
}

beta() {
  sum=1
  for record in "$@"; do
    if [ "$record" -gt 20 ]; then
      sum=$((sum + record * 2))
    else
      sum=$((sum + record))
    fi
  done
  echo "$sum"
}

gamma() {
  acc=2
  for row in "$@"; do
    if [ "$row" -gt 30 ]; then
      acc=$((acc + row * 2))
    else
      acc=$((acc + row))
    fi
  done
  echo "$acc"
}
"#;

    let scan =
        scan_similar_functions_report(&[source_file("scripts/deploy.sh", source)], &options())?;

    assert_eq!(scan.candidate_count, 3, "{scan:#?}");
    assert_eq!(scan.detections.len(), 1, "{scan:#?}");
    Ok(())
}

#[test]
fn detects_similar_powershell_functions() -> Result<()> {
    let source = r#"
function Invoke-Alpha($Items) {
  $total = 0
  foreach ($item in $Items) {
    if ($item.Score -gt 10) {
      $total += $item.Score * 2
    } else {
      $total += $item.Score
    }
  }
  return $total
}

function Invoke-Beta($Records) {
  $sum = 1
  foreach ($record in $Records) {
    if ($record.Score -gt 20) {
      $sum += $record.Score * 2
    } else {
      $sum += $record.Score
    }
  }
  return $sum
}

function Invoke-Gamma($Rows) {
  $acc = 2
  foreach ($row in $Rows) {
    if ($row.Score -gt 30) {
      $acc += $row.Score * 2
    } else {
      $acc += $row.Score
    }
  }
  return $acc
}
"#;

    let scan =
        scan_similar_functions_report(&[source_file("scripts/deploy.ps1", source)], &options())?;

    assert_eq!(scan.candidate_count, 3, "{scan:#?}");
    assert_eq!(scan.detections.len(), 1, "{scan:#?}");
    Ok(())
}
