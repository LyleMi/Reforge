fn detected_language(path: &Path) -> Option<String> {
    const EXTENSION_LANGUAGES: &[(&str, &str)] = &[
        ("rs", "rust"),
        ("js", "javascript"),
        ("jsx", "javascript"),
        ("mjs", "javascript"),
        ("cjs", "javascript"),
        ("ts", "typescript"),
        ("tsx", "tsx"),
        ("vue", "tsx"),
        ("mts", "typescript"),
        ("cts", "typescript"),
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
