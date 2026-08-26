use std::path::PathBuf;

use super::*;

fn source_file(path: &str, source: &str) -> SourceFile {
    SourceFile {
        path: PathBuf::from(path),
        display_path: path.to_string(),
        source: source.into(),
    }
}

fn options() -> StructureOptions {
    StructureOptions {
        max_function_lines: usize::MAX,
        max_function_complexity: usize::MAX,
        max_nesting_depth: usize::MAX,
        max_function_parameters: usize::MAX,
        max_type_lines: usize::MAX,
        max_type_members: usize::MAX,
        max_imports: usize::MAX,
        max_public_items: usize::MAX,
        max_functions_per_file: usize::MAX,
        max_functions_per_100_lines: usize::MAX,
        max_small_function_ratio: usize::MAX,
        min_module_functions: usize::MAX,
        min_clustered_function_percent: 100,
        min_repeated_literal_occurrences: usize::MAX,
        min_data_clump_occurrences: usize::MAX,
        max_dir_files: usize::MAX,
        include_test_structure: false,
    }
}

fn collected_functions(path: &str, source: &str) -> Result<Vec<FunctionMetric>> {
    let parsed = parse_source_files(&[source_file(path, source)])?;
    let file = &parsed[0];
    let mut signals = FileSignals::default();
    Ok(collect_production_ast_signals(
        &file.file,
        file.tree.root_node(),
        StructureTraversal {
            source: &file.file.source,
            family: file.family,
            include_test_structure: true,
        },
        &mut signals,
    )
    .functions)
}

#[test]
fn classifies_javascript_callable_scope_and_names() -> Result<()> {
    let functions = collected_functions(
        "src/app.ts",
        r#"
export function topLevel() {}
export const namedArrow = () => {};
[1].map(() => {});
function outer() {
  function nested() {}
  [1].map(() => nested());
}
class Controller { method() {} }
"#,
    )?;
    let named_arrow = functions
        .iter()
        .find(|item| item.name == "namedArrow")
        .unwrap();
    assert!(named_arrow.is_module_scope);
    assert!(!named_arrow.is_anonymous);
    assert_eq!(named_arrow.callable_depth, 0);
    let nested = functions.iter().find(|item| item.name == "nested").unwrap();
    assert_eq!(nested.callable_depth, 1);
    assert!(!nested.is_module_scope);
    assert!(
        !functions
            .iter()
            .find(|item| item.name == "method")
            .unwrap()
            .is_module_scope
    );
    assert_eq!(functions.iter().filter(|item| item.is_anonymous).count(), 2);
    Ok(())
}

#[test]
fn parses_module_callables_from_tsx_and_vue_scripts() -> Result<()> {
    let tsx = collected_functions(
        "src/View.tsx",
        "const renderCard = () => <section />; function renderView() { return renderCard(); }",
    )?;
    assert!(
        tsx.iter()
            .any(|item| item.name == "renderCard" && item.is_module_scope)
    );
    let vue = collected_functions(
        "src/View.vue",
        "<template><main /></template>\n<script setup lang=\"ts\">\nconst renderCard = () => 1;\nfunction renderView() { return renderCard(); }\n</script>",
    )?;
    assert!(
        vue.iter()
            .any(|item| item.name == "renderCard" && item.is_module_scope)
    );
    Ok(())
}

#[test]
fn function_proliferation_ignores_callbacks_but_counts_top_level_helpers() -> Result<()> {
    let mut thresholds = options();
    thresholds.max_functions_per_file = 2;
    thresholds.max_functions_per_100_lines = 1;
    thresholds.max_small_function_ratio = 50;
    let callbacks = "function mount() { items.map(item => item.id).filter(item => item.active); }";
    let callback_detections =
        scan_structure(&[source_file("src/callbacks.js", callbacks)], &thresholds)?;
    assert!(
        callback_detections
            .iter()
            .all(|item| item.kind != Rule::FunctionProliferation)
    );
    let helpers = "function one() {}\nfunction two() {}\nfunction three() {}\n";
    let helper_detections = scan_structure(&[source_file("src/helpers.js", helpers)], &thresholds)?;
    assert!(
        helper_detections
            .iter()
            .any(|item| item.kind == Rule::FunctionProliferation)
    );
    Ok(())
}

fn cohesion_source() -> &'static str {
    "function renderPage() { renderHeader(); renderBody(); }\nfunction renderHeader() {}\nfunction renderBody() {}\nfunction diffPage() { diffHeader(); diffBody(); }\nfunction diffHeader() {}\nfunction diffBody() {}\n"
}

fn cohesion_options() -> StructureOptions {
    let mut thresholds = options();
    thresholds.min_module_functions = 6;
    thresholds.min_clustered_function_percent = 100;
    thresholds
}

#[test]
fn reports_two_module_responsibility_clusters_deterministically() -> Result<()> {
    let detections = scan_structure(
        &[source_file("src/monolith.js", cohesion_source())],
        &cohesion_options(),
    )?;
    let detection = detections
        .iter()
        .find(|item| item.kind == Rule::LowModuleCohesion)
        .expect("two responsibility clusters should be reported");
    assert_eq!(detection.related_locations.len(), 6);
    assert_eq!(
        detection.related_locations[0].name.as_deref(),
        Some("renderPage")
    );
    assert_eq!(
        detection.related_locations[3].name.as_deref(),
        Some("diffPage")
    );
    assert!(
        detection.message.contains("render:"),
        "{}",
        detection.message
    );
    assert!(detection.message.contains("diff:"), "{}", detection.message);
    assert_eq!(detection.metrics[0].name, MetricId::FileModuleFunctionCount);
    assert_eq!(
        detection.metrics[1].name,
        MetricId::FileResponsibilityClusterCount
    );
    assert_eq!(
        detection.metrics[2].name,
        MetricId::FileClusteredFunctionPercent
    );
    Ok(())
}

#[test]
fn skips_split_modules_router_controller_callbacks_and_python() -> Result<()> {
    let split = [
        source_file(
            "src/render.js",
            &cohesion_source()
                .lines()
                .take(3)
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        source_file(
            "src/diff.js",
            &cohesion_source()
                .lines()
                .skip(3)
                .collect::<Vec<_>>()
                .join("\n"),
        ),
    ];
    let router = source_file(
        "src/router.js",
        "function route() { userHandler(); orderHandler(); auditHandler(); }\nfunction userHandler() {}\nfunction orderHandler() {}\nfunction auditHandler() {}\nfunction registryEntry() {}\nfunction controllerAction() {}",
    );
    let callbacks = source_file(
        "src/View.tsx",
        "function renderView() { return items.map(item => item.children.map(child => <div>{child}</div>)); }",
    );
    let unresolved_calls = source_file(
        "src/dynamic.js",
        "function renderPage(renderHeader) { renderHeader(); this.renderBody(); registry.renderItem(); }\nfunction renderHeader() {}\nfunction renderBody() {}\nfunction diffPage() { tools.diffHeader(); diffBody.call(null); }\nfunction diffHeader() {}\nfunction diffBody() {}",
    );
    let python = source_file("src/monolith.py", "def render_page():\n    pass\n");
    for files in [
        split.to_vec(),
        vec![router],
        vec![callbacks],
        vec![unresolved_calls],
        vec![python],
    ] {
        let detections = scan_structure(&files, &cohesion_options())?;
        assert!(
            detections
                .iter()
                .all(|item| item.kind != Rule::LowModuleCohesion)
        );
    }
    Ok(())
}
