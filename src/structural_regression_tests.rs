use std::path::{Path, PathBuf};

use super::*;

fn source_file(path: &str, source: &str) -> SourceFile {
    SourceFile {
        path: PathBuf::from(path),
        display_path: path.to_string(),
        source: source.into(),
    }
}

#[test]
fn counts_python_parameters_without_annotation_or_default_identifiers() -> Result<()> {
    let source = r#"
def send_file(
    path_or_file: os.PathLike[t.AnyStr] | str | t.IO[bytes],
    mimetype: str | None = None,
    as_attachment: bool = False,
    download_name: str | None = None,
    conditional: bool = True,
    etag: bool | str = True,
    last_modified: datetime | int | float | None = None,
    max_age: None | (int | t.Callable[[str | None], int | None]) = None,
) -> Response:
    pass
"#;

    let parsed = parse_source_files(&[source_file("src/app.py", source)])?;
    let metrics = collect_raw_structure_metrics(&parsed);

    assert_eq!(metrics[0].functions[0].parameter_count, 8);
    Ok(())
}

#[test]
fn treats_java_package_metadata_as_naming_neutral() {
    assert_eq!(
        normalized_naming_stem(Path::new("src/app/package-info.java")),
        None
    );
    assert_eq!(
        normalized_naming_stem(Path::new("src/app/module-info.java")),
        None
    );
}
