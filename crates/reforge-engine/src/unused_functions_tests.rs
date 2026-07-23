use super::*;
use std::path::PathBuf;

fn source_file(path: &str, source: &str) -> SourceFile {
    SourceFile {
        path: PathBuf::from(path),
        display_path: path.to_string(),
        source: source.into(),
    }
}

fn options() -> UnusedFunctionOptions {
    UnusedFunctionOptions {
        include_tests: false,
    }
}

fn unused_names(detections: &[DetectedEvidence]) -> Vec<String> {
    detections
        .iter()
        .filter(|detection| detection.kind == Rule::UnusedFunction)
        .map(|detection| detection.message.clone())
        .collect()
}

#[test]
fn reports_private_rust_functions_without_references() -> Result<()> {
    let source = r#"
fn used() {}

fn dead() {}

pub fn exported() {
    caller();
}

fn caller() {
    used();
}
"#;

    let detections = scan_unused_functions(&[source_file("src/lib.rs", source)], &options())?;
    let names = unused_names(&detections);

    assert_eq!(detections.len(), 1, "{detections:#?}");
    assert!(names[0].contains("`dead`"));
    assert_eq!(detections[0].line, Some(4));
    assert_eq!(detections[0].metrics[0].name, MetricId::FunctionReferences);
    assert_eq!(detections[0].metrics[0].value, 0);
    Ok(())
}

#[test]
fn counts_only_recognized_rust_serde_callback_paths_as_references() -> Result<()> {
    let source = r#"
fn write_value<S>(value: &String, serializer: S) -> Result<S::Ok, S::Error> where S: serde::Serializer { serializer.serialize_str(value) }
fn read_value<'de, D>(deserializer: D) -> Result<String, D::Error> where D: serde::Deserializer<'de> { String::deserialize(deserializer) }
fn ignored_string() {}

#[derive(serde::Serialize, serde::Deserialize)]
struct Payload {
    #[serde(serialize_with = "crate::write_value", deserialize_with = "read_value")]
    value: String,
    note: &'static str,
}

const NOTE: &str = "ignored_string";
"#;
    let detections = scan_unused_functions(&[source_file("src/lib.rs", source)], &options())?;
    let names = unused_names(&detections);
    assert!(
        names
            .iter()
            .all(|name| !name.contains("write_value") && !name.contains("read_value")),
        "{detections:#?}"
    );
    assert!(
        names.iter().any(|name| name.contains("ignored_string")),
        "{detections:#?}"
    );
    Ok(())
}

#[test]
fn reports_recursive_function_when_only_self_referenced() -> Result<()> {
    let source = r#"
fn recursive(value: usize) -> usize {
    if value == 0 {
        return 0;
    }
    recursive(value - 1)
}
"#;

    let detections = scan_unused_functions(&[source_file("src/lib.rs", source)], &options())?;

    assert_eq!(detections.len(), 1, "{detections:#?}");
    assert!(detections[0].message.contains("`recursive`"));
    Ok(())
}

#[test]
fn counts_rust_test_references_but_skips_test_helpers_by_default() -> Result<()> {
    let source = r#"
fn helper() -> usize {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_helper() -> usize {
        2
    }

    #[test]
    fn calls_helper() {
        assert_eq!(helper(), 1);
        assert_eq!(test_helper(), 2);
    }
}
"#;

    let detections = scan_unused_functions(&[source_file("src/lib.rs", source)], &options())?;

    assert!(detections.is_empty(), "{detections:#?}");
    Ok(())
}

#[test]
fn reports_local_typescript_functions_and_skips_exports() -> Result<()> {
    let source = r#"
export function routeHandler() {
  caller();
}

function usedHelper() {}

function staleHelper() {}

function caller() {
  usedHelper();
}
"#;

    let detections = scan_unused_functions(&[source_file("src/app.ts", source)], &options())?;

    assert_eq!(detections.len(), 1, "{detections:#?}");
    assert!(detections[0].message.contains("`staleHelper`"));
    Ok(())
}

#[test]
fn skips_public_python_functions_but_reports_private_helpers() -> Result<()> {
    let source = r#"
def route_handler():
    return None

def _unused_helper():
    return None
"#;

    let detections = scan_unused_functions(&[source_file("src/app.py", source)], &options())?;

    assert_eq!(detections.len(), 1, "{detections:#?}");
    assert!(detections[0].message.contains("`_unused_helper`"));
    Ok(())
}

#[test]
fn reports_unused_csharp_local_functions() -> Result<()> {
    let source = r#"
class Worker {
    void Run() {
        int Used(int value) => value + 1;
        int Stale(int value) => value + 2;
        System.Console.WriteLine(Used(1));
    }
}
"#;

    let detections = scan_unused_functions(&[source_file("src/Worker.cs", source)], &options())?;

    assert_eq!(detections.len(), 1, "{detections:#?}");
    assert!(detections[0].message.contains("`Stale`"));
    Ok(())
}
