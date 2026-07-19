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

    fn scan_assemblies(&mut self) -> Result<()> {
        self.load_assemblies();
        let edges = assembly_edges(&self.assemblies);
        let local_names = local_assembly_names(&self.assemblies);
        self.report.assembly_edges = edges
            .iter()
            .filter(|edge| local_names.contains(&edge.from))
            .cloned()
            .collect();
        self.report.assemblies = self
            .assemblies
            .iter()
            .filter(|assembly| local_names.contains(&assembly.node.name))
            .map(|assembly| assembly.node.clone())
            .collect();
        let nodes = self
            .assemblies
            .iter()
            .map(|assembly| assembly.node.clone())
            .collect::<Vec<_>>();
        for assembly in nodes.iter().filter(|node| local_names.contains(&node.name)) {
            self.scan_assembly_dependencies(assembly, &nodes, &edges);
        }
        let report_edges = self.report.assembly_edges.clone();
        self.find_assembly_cycles(&report_edges);
        Ok(())
    }

    fn load_assemblies(&mut self) {
        let paths = self
            .paths
            .iter()
            .chain(self.external_paths.iter())
            .filter(|path| extension(path) == Some("asmdef"))
            .cloned()
            .collect::<Vec<_>>();
        for path in paths {
            let Some(record) = load_assembly_record(self.root, &path) else {
                continue;
            };
            if record.node.test_assembly {
                self.report.stats.tests += 1;
            }
            self.assemblies.push(record);
        }
    }

    fn scan_assembly_dependencies(
        &mut self,
        assembly: &UnityAssemblyNode,
        nodes: &[UnityAssemblyNode],
        edges: &[UnityAssemblyEdge],
    ) {
        let outgoing = edges
            .iter()
            .filter(|edge| edge.from == assembly.name)
            .collect::<Vec<_>>();
        if outgoing.len() > self.args.max_unity_assembly_dependencies {
            self.push_finding(
                FindingKind::UnityAssemblyHub,
                &assembly.path,
                1,
                &format!(
                    "Unity assembly {} has {} direct dependencies",
                    assembly.name,
                    outgoing.len()
                ), (
                outgoing.len(),
                self.args.max_unity_assembly_dependencies));
        }
        self.report.raw_metrics.push(UnityRawMetric {
            name: "unity.assembly.dependencies".into(),
            path: assembly.path.clone(),
            value: outgoing.len(),
            unit: "assemblies".into(),
        });
        for edge in outgoing {
            self.inspect_assembly_edge(assembly, nodes, edge);
        }
    }

    fn inspect_assembly_edge(
        &mut self,
        assembly: &UnityAssemblyNode,
        nodes: &[UnityAssemblyNode],
        edge: &UnityAssemblyEdge,
    ) {
        if !edge.resolved && self.package_cache_present {
            self.push_finding(
                FindingKind::UnityUnresolvedAssemblyReference,
                &assembly.path,
                1,
                &format!(
                    "Unity assembly reference '{}' could not be resolved",
                    edge.reference
                ), (
                1,
                1));
        }
        let editor_target = edge.to.starts_with("UnityEditor")
            || nodes
                .iter()
                .find(|candidate| candidate.name == edge.to)
                .is_some_and(|target| target.editor_only);
        if !assembly.editor_only && editor_target {
            self.push_finding(
                FindingKind::UnityRuntimeEditorDependency,
                &assembly.path,
                1,
                &format!(
                    "runtime Unity assembly {} depends on Editor-only assembly {}",
                    assembly.name, edge.to
                ), (
                1,
                1));
        }
    }

    fn find_assembly_cycles(&mut self, edges: &[UnityAssemblyEdge]) {
        for members in assembly_cycles(edges) {
            self.findings
                .push(assembly_cycle_finding(&members, &self.assemblies));
        }
    }

    fn scan_csharp(&mut self) -> Result<()> {
        let files = self
            .paths
            .iter()
            .filter(|path| extension(path) == Some("cs"))
            .cloned()
            .collect::<Vec<_>>();
        let (base_by_type, records) = load_csharp_records(&files);
        let unity_types = base_by_type
            .keys()
            .filter(|name| inherits_unity(name, &base_by_type, &mut BTreeSet::new()))
            .cloned()
            .collect::<BTreeSet<_>>();
        for record in records {
            self.scan_csharp_record(record, &unity_types);
        }
        Ok(())
    }

    fn scan_csharp_record(&mut self, record: CSharpRecord, unity_types: &BTreeSet<String>) {
        let display = display_path(self.root, &record.path);
        if !is_editor_path(&display) {
            scan_editor_api(&record.source, &display, &mut self.findings);
        }
        if contains_test_attribute(&record.source) {
            self.report.stats.tests += 1;
        }
        let unity_type_names = record
            .declared_types
            .into_iter()
            .filter(|name| unity_types.contains(name))
            .collect::<Vec<_>>();
        if unity_type_names.is_empty() {
            return;
        }
        let type_label = unity_type_names.join(", ");
        let serialized_fields = serialized_field_count(&record.source);
        let lifecycle = lifecycle_method_count(&record.source);
        self.record_unity_type_metrics(&display, serialized_fields, lifecycle);
        if serialized_fields > self.args.max_unity_serialized_fields {
            self.push_finding(
                FindingKind::UnitySerializedFieldBloat,
                &display,
                1,
                &format!(
                    "Unity file containing {type_label} has {serialized_fields} serialized fields"
                ), (
                serialized_fields,
                self.args.max_unity_serialized_fields));
        }
        if lifecycle > self.args.max_unity_lifecycle_methods {
            self.push_finding(
                FindingKind::UnityLifecycleOverload,
                &display,
                1,
                &format!(
                    "Unity file containing {type_label} implements {lifecycle} lifecycle methods"
                ), (
                lifecycle,
                self.args.max_unity_lifecycle_methods));
        }
        scan_frame_calls(&record.source, &display, &mut self.findings);
        scan_event_balance(&record.source, &display, &mut self.findings);
    }

    fn record_unity_type_metrics(&mut self, path: &str, fields: usize, lifecycle: usize) {
        self.report.raw_metrics.push(UnityRawMetric {
            name: "unity.type.serialized_fields".into(),
            path: path.into(),
            value: fields,
            unit: "fields".into(),
        });
        self.report.raw_metrics.push(UnityRawMetric {
            name: "unity.type.lifecycle_methods".into(),
            path: path.into(),
            value: lifecycle,
            unit: "methods".into(),
        });
    }

    fn scan_build_settings(&mut self) -> Result<()> {
        let build = fs::read_to_string(self.root.join("ProjectSettings/EditorBuildSettings.asset"))
            .unwrap_or_default();
        let included = included_build_scenes(&build);
        let scenes = self
            .paths
            .iter()
            .filter(|path| extension(path) == Some("unity"))
            .map(|path| display_path(self.root, path))
            .filter(|path| path.starts_with("Assets/"))
            .collect::<Vec<_>>();
        for scene in scenes {
            if !included.contains(&scene) {
                self.push_finding(
                    FindingKind::UnitySceneBuildDrift,
                    &scene,
                    1,
                    "Unity scene is not listed in EditorBuildSettings", (
                    1,
                    1));
            }
        }
        Ok(())
    }

    fn coverage(&self) -> Vec<UnityCoverageEntry> {
        vec![
            UnityCoverageEntry {
                area: "assemblies".into(),
                status: if self.package_cache_present {
                    UnityProjectStatus::Observed
                } else {
                    UnityProjectStatus::PartiallyObserved
                },
                reason: (!self.package_cache_present)
                    .then(|| "external package assemblies unavailable".into()),
            },
            UnityCoverageEntry {
                area: "asset_references".into(),
                status: self.report.status,
                reason: self.report.degraded_reasons.first().cloned(),
            },
            UnityCoverageEntry {
                area: "csharp_semantics".into(),
                status: UnityProjectStatus::Observed,
                reason: None,
            },
        ]
    }

    fn degrade(&mut self, reason: &str) {
        self.report.status = UnityProjectStatus::PartiallyObserved;
        if !self
            .report
            .degraded_reasons
            .iter()
            .any(|value| value == reason)
        {
            self.report.degraded_reasons.push(reason.into());
        }
    }

    fn push_finding(
        &mut self,
        kind: FindingKind,
        path: &str,
        line: usize,
        message: &str,
        metric: (usize, usize),
    ) {
        self.findings.push(unity_finding(UnityFindingInput::new(
            kind,
            path.to_string(),
            line,
            message.to_string(),
            metric,
        )));
    }
}
