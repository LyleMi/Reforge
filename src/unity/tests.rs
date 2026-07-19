#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("reforge-unity-{name}-{suffix}"))
    }

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn meta(guid: &str) -> String {
        format!("fileFormatVersion: 2\nguid: {guid}\n")
    }

    fn unity_project(name: &str) -> PathBuf {
        let root = test_root(name);
        write(
            &root.join("ProjectSettings/ProjectVersion.txt"),
            "m_EditorVersion: 2022.3.62f1\n",
        );
        write(
            &root.join("ProjectSettings/EditorSettings.asset"),
            "%YAML 1.1\nm_SerializationMode: 2\n",
        );
        write(
            &root.join("ProjectSettings/EditorBuildSettings.asset"),
            "EditorBuildSettings:\n  m_Scenes:\n  - enabled: 1\n    path: Assets/Main.unity\n",
        );
        root
    }

    #[test]
    fn parses_meta_guids_case_insensitively() {
        assert_eq!(
            meta_guid("fileFormatVersion: 2\nguid: AABBCCDDEEFF00112233445566778899\n").as_deref(),
            Some("aabbccddeeff00112233445566778899")
        );
    }

    #[test]
    fn detects_indirect_unity_inheritance() {
        let bases = BTreeMap::from([
            ("Base".into(), "MonoBehaviour".into()),
            ("Game".into(), "Base".into()),
        ]);
        assert!(inherits_unity("Game", &bases, &mut BTreeSet::new()));
    }

    #[test]
    fn auto_scans_text_assets_asmdefs_and_unity_csharp() -> Result<()> {
        let root = unity_project("complete");
        write(
            &root.join("Assets/Core.asmdef"),
            r#"{"name":"Game.Core","references":[]}"#,
        );
        write(
            &root.join("Assets/Core.asmdef.meta"),
            &meta("11111111111111111111111111111111"),
        );
        write(
            &root.join("Assets/Game.cs"),
            "using UnityEngine;\npublic class Game : MonoBehaviour {\n[SerializeField] private int score;\nvoid Update() { Tick(); }\nvoid Tick() { Resources.Load(\"card\"); }\n}\n",
        );
        write(
            &root.join("Assets/Game.cs.meta"),
            &meta("22222222222222222222222222222222"),
        );
        write(
            &root.join("Assets/Main.unity"),
            "%YAML 1.1\n--- !u!1 &1\nGameObject:\n--- !u!114 &2\nMonoBehaviour:\n  m_Script: {fileID: 11500000, guid: 22222222222222222222222222222222, type: 3}\n",
        );
        write(
            &root.join("Assets/Main.unity.meta"),
            &meta("33333333333333333333333333333333"),
        );
        let args = ScanArgs::defaults_for_path(root.clone());

        let scan = scan_unity(&root, &args)?;

        fs::remove_dir_all(&root)?;
        assert_eq!(scan.report.editor_version.as_deref(), Some("2022.3.62f1"));
        assert_eq!(
            scan.report.serialization_mode.as_deref(),
            Some("force_text")
        );
        assert_eq!(scan.report.stats.scenes, 1);
        assert!(
            scan.report
                .assemblies
                .iter()
                .any(|assembly| assembly.name == "Game.Core")
        );
        assert!(
            scan.findings
                .iter()
                .any(|finding| finding.kind == FindingKind::UnityExpensiveFrameCall)
        );
        assert!(!scan.findings.iter().any(|finding| matches!(
            finding.kind,
            FindingKind::UnityBrokenAssetReference | FindingKind::UnityMissingScript
        )));
        assert_eq!(scan.report.status, UnityProjectStatus::PartiallyObserved);
        Ok(())
    }

    #[test]
    fn package_cache_enables_definitive_broken_reference_findings() -> Result<()> {
        let root = unity_project("broken");
        fs::create_dir_all(root.join("Library/PackageCache"))?;
        write(
            &root.join("Assets/Broken.prefab"),
            "%YAML 1.1\n--- !u!114 &1\nMonoBehaviour:\n  m_Script: {fileID: 11500000, guid: ffffffffffffffffffffffffffffffff, type: 3}\n",
        );
        write(
            &root.join("Assets/Broken.prefab.meta"),
            &meta("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        );
        let args = ScanArgs::defaults_for_path(root.clone());

        let scan = scan_unity(&root, &args)?;

        fs::remove_dir_all(&root)?;
        assert_eq!(scan.report.status, UnityProjectStatus::Observed);
        assert!(
            scan.findings
                .iter()
                .any(|finding| finding.kind == FindingKind::UnityMissingScript)
        );
        assert_eq!(scan.report.problem_references.len(), 1);
        Ok(())
    }

    #[test]
    fn scans_each_csharp_file_once_and_checks_non_behaviour_editor_api() -> Result<()> {
        let root = unity_project("csharp-file-scope");
        write(
            &root.join("Assets/Behaviours.cs"),
            "using UnityEngine;\npublic class First : MonoBehaviour { void Update() { Resources.Load(\"card\"); } }\n[Test]\npublic class Second : MonoBehaviour {}\n",
        );
        write(
            &root.join("Assets/RuntimeHelper.cs"),
            "public class RuntimeHelper { UnityEditor.Editor editor; }\n",
        );
        let args = ScanArgs::defaults_for_path(root.clone());

        let scan = scan_unity(&root, &args)?;

        fs::remove_dir_all(&root)?;
        assert_eq!(
            scan.findings
                .iter()
                .filter(|finding| finding.kind == FindingKind::UnityExpensiveFrameCall)
                .count(),
            1
        );
        assert_eq!(
            scan.findings
                .iter()
                .filter(|finding| finding.kind == FindingKind::UnityEditorApiInRuntime)
                .count(),
            1
        );
        assert_eq!(scan.report.stats.tests, 1);
        assert_eq!(scan.report.raw_metrics.len(), 2);
        Ok(())
    }

    #[test]
    fn editor_api_guards_handle_else_and_nested_directives() {
        let source = "#if UNITY_EDITOR\nusing UnityEditor;\n#if DEBUG\nUnityEditor.Editor editor;\n#endif\n#else\nUnityEditor.Editor runtimeEditor;\n#endif\n";
        let mut findings = Vec::new();

        scan_editor_api(source, "Assets/Runtime.cs", &mut findings);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, Some(7));
    }

    #[test]
    fn disabled_build_settings_scenes_are_reported_as_drift() -> Result<()> {
        let root = unity_project("disabled-scene");
        write(
            &root.join("ProjectSettings/EditorBuildSettings.asset"),
            "EditorBuildSettings:\n  m_Scenes:\n  - enabled: 0\n    path: Assets/Main.unity\n",
        );
        write(
            &root.join("Assets/Main.unity"),
            "%YAML 1.1\n--- !u!1 &1\nGameObject:\n",
        );
        let args = ScanArgs::defaults_for_path(root.clone());

        let scan = scan_unity(&root, &args)?;

        fs::remove_dir_all(&root)?;
        assert!(
            scan.findings
                .iter()
                .any(|finding| finding.kind == FindingKind::UnitySceneBuildDrift)
        );
        Ok(())
    }

    #[test]
    fn unity_on_requires_a_project_root_and_off_records_disabled() -> Result<()> {
        let root = test_root("modes");
        fs::create_dir_all(&root)?;
        let mut args = ScanArgs::defaults_for_path(root.clone());
        args.unity = UnityMode::On;
        assert!(
            scan_unity(&root, &args)
                .unwrap_err()
                .to_string()
                .contains("not a Unity project root")
        );
        let project = unity_project("disabled");
        args = ScanArgs::defaults_for_path(project.clone());
        args.unity = UnityMode::Off;
        assert_eq!(
            scan_unity(&project, &args)?.report.status,
            UnityProjectStatus::Disabled
        );
        fs::remove_dir_all(root)?;
        fs::remove_dir_all(project)?;
        Ok(())
    }
}
