fn project_issues(detections: Vec<Detection>) -> Vec<Issue> {
    let mut groups = BTreeMap::<(String, Subject), Vec<Evidence>>::new();
    for detection in detections {
        let definition = RULES
            .iter()
            .find(|rule| rule.name == detection.rule)
            .expect("Unity rule is registered");
        let subject = detection_subject(definition.subject, &detection);
        let evidence = project_evidence(detection);
        groups
            .entry((qualified_family(definition.family), subject.canonicalized()))
            .or_default()
            .push(evidence);
    }
    groups
        .into_iter()
        .map(|((family, subject), evidence)| project_issue(family, subject, evidence))
        .collect()
}

fn detection_subject(kind: SubjectKind, detection: &Detection) -> Subject {
    match kind {
        SubjectKind::File => Subject::File {
            path: detection.path.clone(),
        },
        SubjectKind::Symbol => Subject::Symbol {
            path: detection.path.clone(),
            symbol: detection
                .related
                .first()
                .map(|(_, name)| name.clone())
                .unwrap_or_else(|| "Unity type".into()),
        },
        SubjectKind::Group => Subject::Group {
            members: related_members(&detection.related),
        },
    }
}

fn related_members(related: &[(String, String)]) -> Vec<String> {
    related
        .iter()
        .map(|(path, name)| format!("{path}#{name}"))
        .collect()
}

fn project_evidence(detection: Detection) -> Evidence {
    let anchor = if detection.related.is_empty() {
        format!("{}:{}", detection.rule, detection.path)
    } else {
        format!(
            "{}:{}",
            detection.rule,
            related_members(&detection.related).join("|")
        )
    };
    let mut evidence = Evidence::new(
        qualified_rule(detection.rule),
        &anchor,
        detection.message,
    );
    evidence.measurements.push(Measurement {
        name: "group.size".into(),
        value: detection.value as f64,
        threshold: Some(detection.threshold as f64),
        unit: "items".into(),
    });
    evidence.locations.push(Location {
        path: detection.path,
        line: Some(detection.line),
        symbol: None,
    });
    evidence.locations.extend(
        detection
            .related
            .into_iter()
            .map(|(path, symbol)| Location {
                path,
                line: Some(1),
                symbol: Some(symbol),
            }),
    );
    evidence
}

fn project_issue(family: String, subject: Subject, evidence: Vec<Evidence>) -> Issue {
    let family_name = family.rsplit('.').next().unwrap_or("Unity issue");
    Issue::new(
        ANALYSIS_UNITY,
        family.clone(),
        subject.clone(),
        (
            format!(
                "{}: {}",
                family_name.replace('_', " "),
                subject.display_name()
            ),
            guidance(family_name),
        ),
        evidence,
    )
}

macro_rules! detection {
    ($rule:expr, $path:expr, $line:expr, $message:expr, $value:expr, $threshold:expr $(,)?) => {
        Detection {
            rule: $rule,
            path: $path.into(),
            line: $line,
            message: $message,
            value: $value,
            threshold: $threshold,
            related: Vec::new(),
        }
    };
}

macro_rules! push {
    ($scan:expr, $rule:expr, $path:expr, $line:expr, $message:expr, $value:expr, $threshold:expr $(,)?) => {
        $scan.detections.push(detection!(
            $rule,
            $path,
            $line,
            $message.into(),
            $value,
            $threshold,
        ))
    };
}

fn qualified_rule(rule: &str) -> String {
    format!("reforge.unity.{rule}")
}
fn qualified_family(family: &str) -> String {
    format!("reforge.unity.{family}")
}

fn guidance(family: &str) -> &'static str {
    match family {
        "dependency_topology" => {
            "Reshape Unity assembly dependencies around stable, acyclic ownership boundaries."
        }
        "reference_integrity" => {
            "Repair the Unity reference and keep serialized metadata consistent."
        }
        "runtime_editor_boundary" => {
            "Keep editor-only APIs and dependencies outside runtime assemblies."
        }
        "project_configuration" => {
            "Align project settings and serialized assets with the declared build contract."
        }
        "runtime_performance" => "Move repeated expensive work out of frame-loop paths.",
        "lifecycle_correctness" => {
            "Pair lifecycle responsibilities and subscriptions deterministically."
        }
        _ => "Split the Unity subject around cohesive responsibilities.",
    }
}


fn workspace_identity(root: &Path) -> String {
    let identity = git(root, &["config", "--get", "remote.origin.url"])
        .or_else(|| {
            git(root, &["rev-parse", "--show-toplevel"]).and_then(|top| {
                Path::new(&top)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
        })
        .unwrap_or_else(|| {
            root.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        });
    let digest = Sha256::digest(
        identity
            .trim_end_matches(".git")
            .replace('\\', "/")
            .as_bytes(),
    );
    format!("rw5-{}", &format!("{digest:x}")[..20])
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}
