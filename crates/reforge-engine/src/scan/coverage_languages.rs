use crate::lang::{JAVASCRIPT_LANGUAGE, TYPESCRIPT_LANGUAGE};

fn detected_language(path: &Path) -> Option<String> {
    const EXTENSION_LANGUAGES: &[(&str, &str)] = &[
        ("rs", "rust"),
        ("js", JAVASCRIPT_LANGUAGE),
        ("jsx", JAVASCRIPT_LANGUAGE),
        ("mjs", JAVASCRIPT_LANGUAGE),
        ("cjs", JAVASCRIPT_LANGUAGE),
        ("ts", TYPESCRIPT_LANGUAGE),
        ("tsx", "tsx"),
        ("vue", "tsx"),
        ("mts", TYPESCRIPT_LANGUAGE),
        ("cts", TYPESCRIPT_LANGUAGE),
        ("py", "python"),
        ("go", "go"),
        ("java", "java"),
        ("cs", "csharp"),
        ("csx", "csharp"),
        ("kt", "kotlin"),
        ("php", "php"),
        ("rb", "ruby"),
        ("sh", "bash"),
        ("bash", "bash"),
        ("ps1", "powershell"),
        ("psm1", "powershell"),
        ("c", "c"),
        ("h", "c"),
        ("cc", "cpp"),
        ("cpp", "cpp"),
        ("cxx", "cpp"),
        ("hh", "cpp"),
        ("hpp", "cpp"),
        ("hxx", "cpp"),
    ];
    let extension = path.extension()?.to_str()?;
    EXTENSION_LANGUAGES
        .iter()
        .find_map(|(candidate, language)| (*candidate == extension).then(|| (*language).into()))
}
