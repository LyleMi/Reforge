fn resolve_scan_root(args: &ScanArgs) -> Result<PathBuf> {
    args.path
        .canonicalize()
        .with_context(|| format!("failed to resolve path {}", args.path.display()))
}

fn report_scan_start(
    progress: &mut dyn ProgressSink,
    root: &Path,
    total_source_files: Option<usize>,
) {
    match total_source_files {
        Some(total) => progress.report(&format!(
            "Scanning {} ({total} source {})",
            display_path(root),
            pluralize(total, "file")
        )),
        None => progress.report(&format!("Scanning {}", display_path(root))),
    }
}

fn scan_sources(
    source_plan: SourceScanPlan,
    args: &ScanArgs,
    total_source_files: Option<usize>,
    progress: &mut dyn ProgressSink,
    scan: &mut SourceScan,
) -> Result<()> {
    scan.stats.directories_scanned = source_plan.directories_scanned;
    for path in &source_plan.source_files {
        let file_options = FileScanOptions {
            max_file_lines: args.max_file_lines,
        };
        scan_file(path, file_options, scan)?;
        report_file_scan_progress(progress, &scan.stats, total_source_files, path);
    }

    scan_directories(
        &source_plan.directory_source_files,
        args.max_dir_files,
        &mut scan.raw_metrics,
        &mut scan.findings,
    );
    Ok(())
}

fn report_file_scan_progress(
    progress: &mut dyn ProgressSink,
    stats: &ScanStats,
    total_source_files: Option<usize>,
    path: &Path,
) {
    if let Some(total) = total_source_files {
        let detail = display_path(path);
        progress.report_scan_progress(ProgressEvent {
            completed: stats.source_files_scanned,
            total,
            detail: &detail,
        });
    }
}

struct ScanSimilarityProgress<'a> {
    progress: &'a mut dyn ProgressSink,
}

impl SimilarFunctionProgress for ScanSimilarityProgress<'_> {
    fn report_extract_progress(&mut self, completed: usize, total: usize, path: &str) {
        self.progress.report_analysis_progress(
            ProgressEvent {
                completed,
                total,
                detail: path,
            },
            "extracting candidates",
        );
    }

    fn report_compare_progress(&mut self, completed: usize, total: usize) {
        self.progress.report_analysis_progress(
            ProgressEvent {
                completed,
                total,
                detail: "",
            },
            "comparing candidates",
        );
    }
}

fn collect_source_scan_plan(root: &Path, args: &ScanArgs) -> Result<SourceScanPlan> {
    let mut plan = SourceScanPlan::default();

    if root.is_file() {
        if is_supported_source(root) && should_scan_source_file(root, args) {
            plan.source_files.push(root.to_path_buf());
        }
        return Ok(plan);
    }

    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(!args.filters.include_hidden)
        .git_ignore(!args.filters.no_gitignore)
        .git_global(!args.filters.no_gitignore)
        .git_exclude(!args.filters.no_gitignore)
        .require_git(false);

    let root_for_filter = root.to_path_buf();
    let args_for_filter = args.clone();
    for entry in builder
        .filter_entry(move |entry| should_visit_entry(entry, &root_for_filter, &args_for_filter))
        .build()
    {
        let entry = entry?;
        let Some(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            plan.directories_scanned += 1;
        } else if file_type.is_file()
            && is_supported_source(entry.path())
            && should_scan_source_file(entry.path(), args)
        {
            let path = entry.path().to_path_buf();
            count_source_file_parent(&path, &mut plan.directory_source_files);
            plan.source_files.push(path);
        }
    }

    Ok(plan)
}

fn scan_file(path: &Path, options: FileScanOptions, scan: &mut SourceScan) -> Result<()> {
    if !is_supported_source(path) {
        return Ok(());
    }

    scan.stats.source_files_scanned += 1;

    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read source file {}", path.display()))?;
    let source: Arc<str> = Arc::from(source);
    let line_count = source.lines().count();
    let display_path = display_path(path);
    let is_test = is_test_source(path)
        || (path.extension().and_then(|value| value.to_str()) == Some("cs")
            && (source.contains("[Test]") || source.contains("[UnityTest]")));

    scan.raw_metrics.files.push(FileRawMetric {
        path: display_path.clone(),
        loc: line_count,
        imports: 0,
        public_items: 0,
        is_test,
        churn: ChurnFileMetric::default(),
    });

    if line_count > options.max_file_lines {
        scan.findings.push(Finding::from(FindingInput::new(
            FindingKind::LargeFile,
            display_path.clone(),
            Some(1),
            format!("file has {line_count} lines; consider splitting responsibilities"),
            vec![FindingMetric::threshold(
                MetricId::FileLoc,
                line_count,
                options.max_file_lines,
                "lines",
            )],
        )));
    }

    for (index, line) in source.lines().enumerate() {
        if has_debt_marker(line) {
            scan.findings.push(Finding::from(FindingInput::new(
                FindingKind::DebtMarker,
                display_path.clone(),
                Some(index + 1),
                "technical-debt marker found",
                Vec::new(),
            )));
        }
    }

    if is_supported_structure_source(path) {
        let source_file = SourceFile {
            path: path.to_path_buf(),
            display_path: display_path.clone(),
            source: Arc::clone(&source),
        };

        match parse_source_file(source_file.clone())? {
            Some(parsed) => scan.parsed_sources.push(parsed),
            None => scan.parse_failures.push(ParseFailure {
                path: display_path.clone(),
                language: detected_language(path).unwrap_or_else(|| "unknown".into()),
                reason: ParseFailureReason::SyntaxError,
            }),
        }

        scan.structure_sources.push(source_file);
    }

    Ok(())
}

fn count_source_file_parent(path: &Path, directory_source_files: &mut BTreeMap<PathBuf, usize>) {
    if let Some(parent) = path.parent() {
        *directory_source_files
            .entry(parent.to_path_buf())
            .or_insert(0) += 1;
    }
}

fn scan_directories(
    directory_source_files: &BTreeMap<PathBuf, usize>,
    max_dir_files: usize,
    raw_metrics: &mut RawMetrics,
    findings: &mut Vec<Finding>,
) {
    for (directory, file_count) in directory_source_files {
        raw_metrics.directories.push(DirectoryRawMetric {
            path: display_path(directory),
            source_files: *file_count,
        });
        if *file_count > max_dir_files {
            findings.push(Finding::from(FindingInput::new(
                FindingKind::LargeDirectory,
                display_path(directory),
                None,
                format!(
                    "directory contains {file_count} source files; consider grouping related responsibilities"
                ),
                vec![FindingMetric::threshold(
                    MetricId::DirectorySourceFiles,
                    *file_count,
                    max_dir_files,
                    "source files",
                )],
            )));
        }
    }
}

fn has_debt_marker(line: &str) -> bool {
    let trimmed = line.trim_start();
    let is_comment = trimmed.starts_with("//")
        || trimmed.starts_with('#')
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || trimmed.starts_with("<!--");

    if !is_comment {
        return false;
    }

    let normalized = trimmed.to_ascii_lowercase();
    normalized.contains("todo") || normalized.contains("fixme")
}

fn is_supported_source(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some(
            "c" | "cc"
                | "cpp"
                | "cs"
                | "csx"
                | "go"
                | "java"
                | "js"
                | "jsx"
                | "mjs"
                | "cjs"
                | "kt"
                | "php"
                | "py"
                | "rb"
                | "rs"
                | "ts"
                | "tsx"
                | "vue"
                | "mts"
                | "cts"
                | "sh"
                | "bash"
                | "ps1"
                | "psm1"
        )
    )
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| name.starts_with('.'))
}

fn is_default_excluded_dir(entry: &DirEntry) -> bool {
    entry
        .file_type()
        .is_some_and(|file_type| file_type.is_dir())
        && entry
            .file_name()
            .to_str()
            .is_some_and(|name| DEFAULT_EXCLUDED_DIRS.contains(&name))
}

fn should_visit_entry(entry: &DirEntry, root: &Path, args: &ScanArgs) -> bool {
    let is_root = entry.path() == root;
    is_root
        || ((args.filters.include_hidden || !is_hidden(entry))
            && (args.filters.include_generated || !is_default_excluded_dir(entry)))
            && !is_ignored_path(entry.path(), root, args)
            && !is_excluded_test_path(entry.path(), args)
}

fn is_ignored_path(path: &Path, root: &Path, args: &ScanArgs) -> bool {
    if args.filters.ignore_paths.is_empty() {
        return false;
    }

    let relative = path
        .strip_prefix(root)
        .ok()
        .map(display_path)
        .unwrap_or_else(|| display_path(path));
    args.filters.ignore_paths.iter().any(|ignore| {
        let ignore = ignore.replace('\\', "/").trim_matches('/').to_string();
        !ignore.is_empty()
            && (relative == ignore
                || relative
                    .strip_prefix(&ignore)
                    .is_some_and(|suffix| suffix.starts_with('/')))
    })
}

fn is_excluded_test_path(path: &Path, args: &ScanArgs) -> bool {
    args.filters.exclude_tests && (is_test_source(path) || csharp_source_declares_tests(path))
}

fn should_scan_source_file(path: &Path, args: &ScanArgs) -> bool {
    !is_excluded_test_path(path, args)
}

fn pluralize(count: usize, noun: &str) -> String {
    if count == 1 {
        noun.to_string()
    } else {
        format!("{noun}s")
    }
}

pub(crate) fn is_test_source(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    if file_name.starts_with("test_")
        || file_name.contains(".test.")
        || file_name.contains(".spec.")
        || file_name.ends_with("_test.go")
        || file_name.ends_with("_test.py")
        || file_name.ends_with("_test.rs")
        || file_name.ends_with("_tests.rs")
    {
        return true;
    }

    path.components().any(|component| {
        component.as_os_str().to_str().is_some_and(|name| {
            let normalized = name.to_ascii_lowercase();
            matches!(
                normalized.as_str(),
                "test" | "tests" | "__tests__" | "spec" | "specs" | "editmode" | "playmode"
            ) || normalized.ends_with("_tests")
                || normalized.ends_with("-tests")
        })
    }) || nearest_asmdef_is_test_assembly(path)
}

fn csharp_source_declares_tests(path: &Path) -> bool {
    path.extension().and_then(|value| value.to_str()) == Some("cs")
        && fs::read_to_string(path)
            .is_ok_and(|source| source.contains("[Test]") || source.contains("[UnityTest]"))
}

fn nearest_asmdef_is_test_assembly(path: &Path) -> bool {
    if path.extension().and_then(|value| value.to_str()) != Some("cs") {
        return false;
    }
    let mut current = path.parent();
    for _ in 0..12 {
        let Some(directory) = current else { break };
        if directory_has_test_asmdef(directory) {
            return true;
        }
        current = directory.parent();
    }
    false
}

fn directory_has_test_asmdef(directory: &Path) -> bool {
    let Ok(entries) = fs::read_dir(directory) else {
        return false;
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|candidate| candidate.extension().and_then(|value| value.to_str()) == Some("asmdef"))
        .any(|asmdef| {
            fs::read_to_string(asmdef).is_ok_and(|source| source.contains("TestAssemblies"))
        })
}

fn display_path(path: &Path) -> String {
    crate::pathing::display_path(path)
}
