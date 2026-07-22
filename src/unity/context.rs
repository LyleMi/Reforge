impl<'a> UnityContext<'a> {
    fn new(root: &'a Path, args: &'a ScanArgs) -> Self {
        Self {
            root,
            args,
            report: UnityProjectReport {
                status: UnityProjectStatus::Observed,
                metric_manifest: unity_metric_manifest(),
                ..Default::default()
            },
            findings: Vec::new(),
            paths: Vec::new(),
            external_paths: Vec::new(),
            guid_paths: BTreeMap::new(),
            assemblies: Vec::new(),
            package_cache_present: false,
        }
    }

    fn scan(&mut self) -> Result<()> {
        self.read_project_settings()?;
        let roots = self.analysis_roots()?;
        for root in &roots {
            collect_files(root, &mut self.paths)?;
            self.report
                .analysis_roots
                .push(display_path(self.root, root));
        }
        let package_cache = self.root.join("Library/PackageCache");
        self.package_cache_present = package_cache.is_dir();
        if self.package_cache_present {
            collect_files(&package_cache, &mut self.external_paths)?;
        } else {
            self.degrade("Library/PackageCache is unavailable; external package GUID and assembly references were not verified");
        }
        self.index_meta(false)?;
        self.index_meta(true)?;
        self.scan_meta_integrity();
        self.scan_assets()?;
        self.scan_assemblies()?;
        self.scan_csharp()?;
        self.scan_build_settings()?;
        self.report.stats.guids = self.guid_paths.len();
        self.report.stats.assemblies = self.report.assemblies.len();
        self.report.coverage = self.coverage();
        Ok(())
    }

    fn read_project_settings(&mut self) -> Result<()> {
        let version = fs::read_to_string(self.root.join("ProjectSettings/ProjectVersion.txt"))
            .context("failed to read Unity ProjectVersion.txt")?;
        self.report.editor_version = version.lines().find_map(|line| {
            line.trim()
                .strip_prefix("m_EditorVersion:")
                .map(str::trim)
                .map(str::to_string)
        });
        let editor_settings =
            fs::read_to_string(self.root.join("ProjectSettings/EditorSettings.asset"))
                .unwrap_or_default();
        let mode = editor_settings.lines().find_map(|line| {
            line.trim()
                .strip_prefix("m_SerializationMode:")
                .map(str::trim)
        });
        self.report.serialization_mode = Some(
            match mode {
                Some("2") => "force_text",
                Some("1") => "force_binary",
                Some("0") => "mixed",
                _ => "unknown",
            }
            .to_string(),
        );
        if mode != Some("2") {
            self.degrade("Unity asset serialization is not Force Text; binary assets cannot be reference-checked");
            self.push_finding(
                FindingKind::UnityNonTextSerialization,
                "ProjectSettings/EditorSettings.asset",
                1,
                "Unity project is not configured for Force Text serialization", (
                1,
                1));
        }
        Ok(())
    }

    fn analysis_roots(&self) -> Result<Vec<PathBuf>> {
        let mut roots = Vec::new();
        let assets = self.root.join("Assets");
        if assets.is_dir() {
            roots.push(assets);
        }
        let packages = self.root.join("Packages");
        for path in package_analysis_roots(&packages)? {
            if !roots.contains(&path) {
                roots.push(path);
            }
        }
        Ok(roots)
    }

    fn index_meta(&mut self, external: bool) -> Result<()> {
        let paths = if external {
            &self.external_paths
        } else {
            &self.paths
        };
        for path in paths.iter().filter(|path| extension(path) == Some("meta")) {
            if !external {
                self.report.stats.meta_files += 1;
            }
            let Ok(contents) = fs::read_to_string(path) else {
                continue;
            };
            let Some(guid) = meta_guid(&contents) else {
                continue;
            };
            let display = display_path(self.root, path);
            self.guid_paths.entry(guid).or_default().push(display);
        }
        Ok(())
    }

    fn scan_meta_integrity(&mut self) {
        self.scan_duplicate_guids();
        self.scan_missing_meta_files();
    }

    fn scan_duplicate_guids(&mut self) {
        let duplicates = self
            .guid_paths
            .values()
            .filter(|paths| paths.len() > 1)
            .cloned()
            .collect::<Vec<_>>();
        for paths in duplicates {
            let local = paths
                .iter()
                .filter(|path| !path.starts_with("Library/PackageCache/"))
                .cloned()
                .collect::<Vec<_>>();
            if local.len() > 1 {
                let related = local
                    .iter()
                    .map(|path| RelatedLocation {
                        path: path.clone(),
                        line: 2,
                        name: Some("duplicate GUID".into()),
                    })
                    .collect();
                self.findings.push(unity_finding(
                    UnityFindingInput::new(
                        FindingKind::UnityDuplicateGuid,
                        local[0].clone(),
                        2,
                        format!("Unity GUID is declared by {} local meta files", local.len()), (
                        local.len(),
                        1))
                    .with_related(related),
                ));
            }
        }
    }

    fn scan_missing_meta_files(&mut self) {
        let local_paths = self.paths.iter().cloned().collect::<BTreeSet<_>>();
        for path in self.paths.clone() {
            if extension(&path) == Some("meta") {
                let target = PathBuf::from(path.to_string_lossy().trim_end_matches(".meta"));
                if !target.exists() {
                    self.push_finding(
                        FindingKind::UnityOrphanMeta,
                        &display_path(self.root, &path),
                        1,
                        "Unity meta file has no matching asset", (
                        1,
                        1));
                }
                continue;
            }
            if path == self.root.join("Assets")
                || path
                    .file_name()
                    .is_some_and(|name| name == "manifest.json" || name == "packages-lock.json")
            {
                continue;
            }
            let meta = PathBuf::from(format!("{}.meta", path.display()));
            if !meta.exists() && local_paths.contains(&path) {
                self.push_finding(
                    FindingKind::UnityMissingMeta,
                    &display_path(self.root, &path),
                    1,
                    "Unity asset has no matching meta file", (
                    1,
                    1));
            }
        }
    }

    fn scan_assets(&mut self) -> Result<()> {
        let local_guids = self.guid_paths.keys().cloned().collect::<BTreeSet<_>>();
        for path in self.paths.clone() {
            if is_scannable_unity_asset(&path) {
                self.scan_asset(&path, &local_guids)?;
            }
        }
        Ok(())
    }

    fn scan_asset(&mut self, path: &Path, local_guids: &BTreeSet<String>) -> Result<()> {
        self.report.stats.assets += 1;
        let bytes = fs::read(path)?;
        let Ok(text) = std::str::from_utf8(&bytes) else {
            self.report.stats.binary_assets += 1;
            self.degrade("one or more Unity assets are binary or not UTF-8 and were counted without reference analysis");
            return Ok(());
        };
        if !text.starts_with("%YAML") && !text.contains("--- !u!") {
            return Ok(());
        }
        self.report.stats.yaml_assets += 1;
        let objects = text
            .lines()
            .filter(|line| line.starts_with("--- !u!"))
            .count();
        self.record_asset_objects(path, objects);
        self.scan_asset_references(path, text, local_guids);
        Ok(())
    }

    fn record_asset_objects(&mut self, path: &Path, objects: usize) {
        let display = display_path(self.root, path);
        let threshold = match extension(path) {
            Some("unity") => {
                self.report.stats.scenes += 1;
                Some((FindingKind::UnityLargeScene, self.args.max_unity_scene_objects, "scene"))
            }
            Some("prefab") => {
                self.report.stats.prefabs += 1;
                Some((FindingKind::UnityLargePrefab, self.args.max_unity_prefab_objects, "prefab"))
            }
            _ => None,
        };
        let Some((kind, limit, label)) = threshold else {
            return;
        };
        if objects > limit {
            self.push_finding(
                kind,
                &display,
                1,
                &format!("Unity {label} contains {objects} serialized objects"), (
                objects,
                limit));
        }
        self.report.raw_metrics.push(UnityRawMetric {
            name: "unity.asset.objects".into(),
            path: display,
            value: objects,
            unit: "objects".into(),
        });
    }

    fn scan_asset_references(
        &mut self,
        path: &Path,
        text: &str,
        local_guids: &BTreeSet<String>,
    ) {
        for (line_index, line) in text.lines().enumerate() {
            for guid in guids_in_line(line) {
                self.record_asset_reference(path, line_index + 1, line, guid, local_guids);
            }
        }
    }

    fn record_asset_reference(
        &mut self,
        path: &Path,
        line_number: usize,
        line: &str,
        guid: String,
        local_guids: &BTreeSet<String>,
    ) {
        self.report.stats.asset_references += 1;
        if guid == ZERO_GUID || local_guids.contains(&guid) || !self.package_cache_present {
            return;
        }
        let display = display_path(self.root, path);
        let is_script = line.contains("m_Script:");
        self.report.problem_references.push(UnityReferenceProblem {
            source_path: display.clone(),
            line: line_number,
            guid,
            file_id: file_id_in_line(line),
            category: if is_script { "script" } else { "asset" }.into(),
            resolved_target: None,
        });
        self.push_finding(
            if is_script {
                FindingKind::UnityMissingScript
            } else {
                FindingKind::UnityBrokenAssetReference
            },
            &display,
            line_number,
            if is_script {
                "Unity asset references a missing MonoScript"
            } else {
                "Unity asset contains an unresolved GUID reference"
            }, (
            1,
            1));
    }

}

include!("context_analysis.rs");
