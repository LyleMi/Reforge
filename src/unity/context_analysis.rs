impl<'a> UnityContext<'a> {
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
