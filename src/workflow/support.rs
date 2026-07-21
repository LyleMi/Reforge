fn resolve_new_run_dir(explicit: Option<&Path>, root: &Path, report_hash: &str) -> Result<PathBuf> {
    let path = explicit.map(Path::to_path_buf).unwrap_or_else(|| {
        root.join(".reforge/runs").join(format!(
            "run-{}-{}",
            epoch_ms() / 1000,
            report_hash
                .trim_start_matches("sha256-")
                .chars()
                .take(12)
                .collect::<String>()
        ))
    });
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()?.join(path)
    };
    if let Some(parent) = absolute.parent() {
        fs::create_dir_all(parent)?;
        let canonical_parent = parent.canonicalize()?;
        let name = absolute
            .file_name()
            .context("run directory must have a final path component")?;
        Ok(canonical_parent.join(name))
    } else {
        bail!("run directory has no parent")
    }
}

fn effective_scan_command(
    root: &Path,
    effective: &Value,
    original: &ScanArgs,
    config_path: Option<&Path>,
) -> Result<Vec<String>> {
    let object = effective
        .as_object()
        .context("effective scan config did not serialize to an object")?;
    let mut command = vec![
        "reforge".to_string(),
        "scan".to_string(),
        portable_path(root),
    ];
    for (key, value) in object {
        append_effective_config(&mut command, key, value)?;
    }
    if let Some(only) = &original.finding_controls.only {
        command.extend(["--only".to_string(), only.clone()]);
    }
    if let Some(excluded) = &original.finding_controls.exclude_detector {
        command.extend(["--exclude-detector".to_string(), excluded.clone()]);
    }
    if let Some(path) = config_path {
        command.extend(["--config".to_string(), portable_path(path)]);
    }
    command.extend(["--progress".to_string(), "never".to_string()]);
    Ok(command)
}

fn append_effective_config(command: &mut Vec<String>, key: &str, value: &Value) -> Result<()> {
    if key == "data-flow" {
        // Data-flow policy remains config-owned; the stored command already retains --config.
        return Ok(());
    }
    if key == "ignore-paths" {
        let values = value
            .as_array()
            .context("ignore-paths effective config must be an array")?;
        for value in values.iter().filter_map(Value::as_str) {
            command.extend(["--ignore-path".to_string(), value.to_string()]);
        }
        return Ok(());
    }
    match value {
        Value::Bool(true) => command.push(format!("--{key}")),
        Value::Bool(false) | Value::Null => {}
        Value::String(value) => command.extend([format!("--{key}"), value.clone()]),
        Value::Number(value) => command.extend([format!("--{key}"), value.to_string()]),
        _ => bail!("unsupported effective config field {key}"),
    }
    Ok(())
}

fn parse_stored_scan_command(command: &[String]) -> Result<ScanArgs> {
    let cli = Cli::try_parse_from_with_explicit_overrides(command)
        .context("stored scan command is no longer valid")?;
    match cli.command {
        Command::Scan(args) => Ok(*args),
        _ => bail!("stored workflow command is not a scan"),
    }
}

fn validate_schema_version(version: u8, artifact: &str) -> Result<()> {
    ensure!(
        version == ARTIFACT_SCHEMA_VERSION,
        "{artifact} uses unsupported artifact schema {version}"
    );
    Ok(())
}

fn validate_paths<'a>(root: &Path, paths: impl Iterator<Item = &'a str>) -> Result<()> {
    for path in paths {
        validate_relative_path(root, path)?;
    }
    Ok(())
}

fn validate_relative_path(root: &Path, path: &str) -> Result<PathBuf> {
    ensure!(!path.trim().is_empty(), "artifact path must not be empty");
    let root = root.canonicalize()?;
    let candidate = Path::new(path);
    ensure!(
        !candidate.is_absolute(),
        "artifact path must be relative: {path}"
    );
    ensure!(
        !candidate.components().any(|component| matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )),
        "artifact path escapes target root: {path}"
    );
    let joined = root.join(candidate);
    let resolved = if joined.exists() {
        joined.canonicalize()?
    } else {
        let mut parent = joined.as_path();
        while !parent.exists() {
            parent = parent
                .parent()
                .context("artifact path has no existing ancestor")?;
        }
        let ancestor = parent.canonicalize()?;
        ensure!(
            ancestor.starts_with(&root),
            "artifact path escapes through a symlink: {path}"
        );
        joined
    };
    ensure!(
        resolved.starts_with(&root),
        "artifact path escapes target root: {path}"
    );
    Ok(resolved)
}

fn ensure_unique<T: Ord + Clone>(values: &[T], label: &str) -> Result<()> {
    let unique = values.iter().cloned().collect::<BTreeSet<_>>();
    ensure!(unique.len() == values.len(), "duplicate {label}");
    Ok(())
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("failed to parse {}", path.display()))
}

fn atomic_write_json<T: Serialize>(path: &Path, value: &T, replace: bool) -> Result<()> {
    if !replace && path.exists() {
        bail!("artifact already exists: {}", path.display());
    }
    let parent = path.parent().context("artifact path has no parent")?;
    fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("artifact path is not valid UTF-8")?;
    let temp = parent.join(format!(".{name}.tmp-{}-{}", std::process::id(), epoch_ms()));
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&temp, &bytes)
        .with_context(|| format!("failed to write temporary artifact {}", temp.display()))?;
    let parsed: Value = serde_json::from_slice(&fs::read(&temp)?)?;
    ensure!(
        parsed.is_object(),
        "serialized workflow artifact is not an object"
    );
    if !replace && path.exists() {
        let _ = fs::remove_file(&temp);
        bail!("artifact already exists: {}", path.display());
    }
    fs::rename(&temp, path)
        .with_context(|| format!("failed to atomically replace {}", path.display()))?;
    Ok(())
}

fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| (key.clone(), canonical_json(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(canonical_json).collect()),
        _ => value.clone(),
    }
}

fn fingerprint_json(value: &Value) -> String {
    let bytes = serde_json::to_vec(&canonical_json(value)).expect("JSON value should serialize");
    hash_bytes(&bytes)
}

fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256-{digest:x}")
}

fn config_fingerprint(effective: &Value, config_path: Option<&Path>) -> Result<String> {
    let source = config_path
        .map(fs::read)
        .transpose()?
        .map(|bytes| hash_bytes(&bytes));
    Ok(fingerprint_json(&serde_json::json!({
        "effective": effective,
        "config_source": source,
    })))
}

fn workspace_snapshot(
    root: &Path,
    excluded_run: Option<&Path>,
) -> Result<BTreeMap<String, String>> {
    let walker_root = root.to_path_buf();
    let excluded_run = excluded_run
        .and_then(|path| path.strip_prefix(root).ok())
        .map(Path::to_path_buf);
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .filter_entry(move |entry| {
            let relative = entry
                .path()
                .strip_prefix(&walker_root)
                .unwrap_or(entry.path());
            !excluded_run
                .as_ref()
                .is_some_and(|excluded| relative == excluded || relative.starts_with(excluded))
                && !relative.components().any(|component| {
                    component.as_os_str() == ".git"
                        || component.as_os_str() == ".reforge"
                        || component.as_os_str() == "target"
                        || component.as_os_str() == "node_modules"
                        || component.as_os_str() == "dist"
                        || component.as_os_str() == "build"
                })
        });
    let mut snapshot = BTreeMap::new();
    for entry in builder.build() {
        let entry = entry?;
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let canonical = entry.path().canonicalize()?;
        ensure!(
            canonical.starts_with(root),
            "workspace symlink escapes target root"
        );
        let relative = entry.path().strip_prefix(root)?;
        snapshot.insert(
            portable_path(relative),
            hash_bytes(&fs::read(entry.path())?),
        );
    }
    Ok(snapshot)
}

fn snapshot_fingerprint(snapshot: &BTreeMap<String, String>) -> Result<String> {
    Ok(fingerprint_json(&serde_json::to_value(snapshot)?))
}

fn snapshot_changes(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> Vec<FileChange> {
    let paths = before
        .keys()
        .chain(after.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    paths
        .into_iter()
        .filter_map(|path| {
            let old = before.get(&path).cloned();
            let new = after.get(&path).cloned();
            (old != new).then_some(FileChange {
                path,
                before_sha256: old,
                after_sha256: new,
            })
        })
        .collect()
}

struct CheckExecution<'a> {
    kind: WorkflowCheckKind,
    program: &'a str,
    args: &'a [String],
    declared: bool,
    root: &'a Path,
    timeout: Duration,
}

fn run_check(execution: CheckExecution<'_>) -> CheckRecord {
    let started = Instant::now();
    let spawn = ProcessCommand::new(execution.program)
        .args(execution.args)
        .current_dir(execution.root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let (command_found, success, timed_out, exit_code, output) = match spawn {
        Err(error) => (false, false, false, None, error.to_string()),
        Ok(mut child) => {
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();
            let stdout_reader = thread::spawn(move || read_stream(stdout));
            let stderr_reader = thread::spawn(move || read_stream(stderr));
            let mut timed_out = false;
            let status = loop {
                match child.try_wait() {
                    Ok(Some(status)) => break Some(status),
                    Ok(None) if started.elapsed() < execution.timeout => {
                        thread::sleep(Duration::from_millis(20));
                    }
                    Ok(None) => {
                        timed_out = true;
                        let _ = child.kill();
                        break child.wait().ok();
                    }
                    Err(_) => break None,
                }
            };
            let mut bytes = stdout_reader.join().unwrap_or_default();
            bytes.extend(stderr_reader.join().unwrap_or_default());
            let output = String::from_utf8_lossy(&bytes).to_string();
            (
                true,
                status.is_some_and(|status| status.success()) && !timed_out,
                timed_out,
                status.and_then(|status| status.code()),
                output,
            )
        }
    };
    CheckRecord {
        kind: execution.kind,
        program: execution.program.to_string(),
        args: execution.args.to_vec(),
        declared: execution.declared,
        command_found,
        success,
        timed_out,
        exit_code,
        duration_ms: started.elapsed().as_millis(),
        output_summary: summarize_output(&output),
        recorded_at_epoch_ms: epoch_ms(),
    }
}

fn read_stream(stream: Option<impl Read>) -> Vec<u8> {
    let mut bytes = Vec::new();
    if let Some(mut stream) = stream {
        let _ = stream.read_to_end(&mut bytes);
    }
    bytes
}

fn summarize_output(output: &str) -> String {
    let redacted = output
        .lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            if ["token=", "password=", "secret=", "api_key=", "apikey="]
                .iter()
                .any(|needle| lower.contains(needle))
            {
                "[redacted]".to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if redacted.len() <= OUTPUT_SUMMARY_LIMIT {
        redacted
    } else {
        let mut boundary = OUTPUT_SUMMARY_LIMIT;
        while !redacted.is_char_boundary(boundary) {
            boundary -= 1;
        }
        format!("{}\n[truncated]", &redacted[..boundary])
    }
}

fn load_verification(dir: &Path) -> Result<VerificationArtifact> {
    let path = dir.join("verification.json");
    if path.exists() {
        let artifact: VerificationArtifact = read_json(&path)?;
        validate_schema_version(artifact.artifact_schema_version, "verification.json")?;
        Ok(artifact)
    } else {
        Ok(VerificationArtifact {
            artifact_schema_version: ARTIFACT_SCHEMA_VERSION,
            checks: Vec::new(),
            result: None,
            reasons: Vec::new(),
            finished_at_epoch_ms: None,
        })
    }
}

fn unobservable_selected_kinds(
    original: &ScanReport,
    current: &ScanReport,
    selected: &BTreeSet<EvidenceId>,
) -> BTreeSet<FindingKind> {
    let selected_kinds = original
        .findings
        .iter()
        .filter(|finding| selected.contains(&finding.id))
        .map(|finding| finding.kind)
        .collect::<BTreeSet<_>>();
    selected_kinds
        .into_iter()
        .filter(|kind| {
            current
                .detector_execution
                .iter()
                .find(|receipt| receipt.kind == *kind)
                .is_none_or(|receipt| {
                    receipt.status != DetectorExecutionStatus::Completed
                        || receipt.unobservable_count > 0
                })
        })
        .collect()
}

fn selected_coverage_limitations(
    report: &ScanReport,
    unobservable_kinds: &BTreeSet<FindingKind>,
) -> Vec<String> {
    let mut limitations = report
        .detector_execution
        .iter()
        .filter(|receipt| unobservable_kinds.contains(&receipt.kind))
        .flat_map(|receipt| receipt.unobservable_reasons.iter().cloned())
        .collect::<Vec<_>>();
    limitations.sort();
    limitations.dedup();
    limitations
}

fn portable_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
