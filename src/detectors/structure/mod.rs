use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{
    ARROW_FUNCTION, BODY_FIELD, FUNCTION_DECLARATION, FUNCTION_DEFINITION, FUNCTION_ITEM,
    GENERATOR_FUNCTION_DECLARATION, LanguageFamily, METHOD_DECLARATION, METHOD_DEFINITION,
    NAME_FIELD, PARAMETERS_FIELD, adapter_for_path, child_by_kind, has_rust_cfg_test_attribute,
};
use crate::model::MetricId;
use crate::scanner::{
    Finding, FindingInput, FindingKind, FindingMetric, RelatedLocation, is_test_source,
};
use crate::similar_functions::{ParsedSourceFile, SourceFile, parse_source_files};

#[derive(Debug, Clone)]
pub struct StructureOptions {
    pub max_function_lines: usize,
    pub max_function_complexity: usize,
    pub max_nesting_depth: usize,
    pub max_function_parameters: usize,
    pub max_type_lines: usize,
    pub max_type_members: usize,
    pub max_imports: usize,
    pub max_public_items: usize,
    pub max_functions_per_file: usize,
    pub max_functions_per_100_lines: usize,
    pub max_small_function_ratio: usize,
    pub min_repeated_literal_occurrences: usize,
    pub min_data_clump_occurrences: usize,
    pub max_dir_files: usize,
    pub include_test_structure: bool,
}

#[derive(Debug, Clone)]
struct FunctionMetric {
    name: String,
    line: usize,
    lines: usize,
    parameter_count: usize,
    parameter_names: Vec<String>,
    complexity: usize,
    nesting_depth: usize,
}

#[derive(Debug, Clone)]
struct TypeMetric {
    name: String,
    line: usize,
    lines: usize,
    members: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawStructureFileMetric {
    pub path: String,
    pub imports: usize,
    pub public_items: usize,
    pub is_test: bool,
    pub functions: Vec<RawStructureFunctionMetric>,
    pub types: Vec<RawStructureTypeMetric>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawStructureFunctionMetric {
    pub path: String,
    pub name: String,
    pub line: usize,
    pub loc: usize,
    pub complexity: usize,
    pub nesting_depth: usize,
    pub parameter_count: usize,
    pub is_test: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawStructureTypeMetric {
    pub path: String,
    pub name: String,
    pub line: usize,
    pub loc: usize,
    pub member_count: usize,
    pub is_test: bool,
}

type Occurrence = RelatedLocation;

const FUNCTION_DENSITY_LINE_UNIT: usize = 100;
const MIN_TEST_SETUP_OCCURRENCES: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FileNamingStyle {
    SnakeCase,
    KebabCase,
    PascalCase,
    CamelCase,
    Lowercase,
    DotSeparated,
    Mixed,
}

#[derive(Debug, Default)]
struct NamingDirectory {
    display_path: String,
    styles: BTreeMap<FileNamingStyle, Vec<Occurrence>>,
}

#[derive(Debug, Default)]
struct FileSignals {
    findings: Vec<Finding>,
    literals: Vec<(String, Occurrence)>,
    error_patterns: Vec<(String, Occurrence)>,
    data_clumps: Vec<(String, Occurrence)>,
    test_setups: Vec<(String, Occurrence)>,
    happy_path_test_files: Vec<(usize, Vec<Occurrence>)>,
    naming_directories: BTreeMap<PathBuf, NamingDirectory>,
    directory_files: BTreeMap<PathBuf, BTreeSet<String>>,
}

#[derive(Debug, Default)]
struct ProductionAstSignals {
    functions: Vec<FunctionMetric>,
    types: Vec<TypeMetric>,
}

#[derive(Debug, Clone, Copy)]
struct StructureTraversal<'a> {
    source: &'a str,
    family: LanguageFamily,
    include_test_structure: bool,
}

struct StructureSignalCollector<'a, 'signals> {
    file: &'a SourceFile,
    traversal: StructureTraversal<'a>,
    signals: &'signals mut FileSignals,
}

#[allow(dead_code)]
pub fn scan_structure(files: &[SourceFile], options: &StructureOptions) -> Result<Vec<Finding>> {
    let parsed_files = parse_source_files(files)?;
    scan_parsed_structure(&parsed_files, options)
}

pub(crate) fn scan_parsed_structure(
    files: &[ParsedSourceFile],
    options: &StructureOptions,
) -> Result<Vec<Finding>> {
    let mut signals = FileSignals::default();

    for file in files {
        collect_file_naming_style(&file.file, &mut signals);

        let is_test = is_test_source(&file.file.path);
        if !is_test || options.include_test_structure {
            scan_production_file(
                &file.file,
                file.family,
                file.tree.root_node(),
                options,
                &mut signals,
            );
        }

        if is_test {
            collect_test_setup_patterns(&file.file, file.tree.root_node(), &mut signals);
            collect_happy_path_test_risk(&file.file, file.family, &mut signals);
        }
    }

    signals.findings.extend(group_occurrences(
        signals.literals,
        options.min_repeated_literal_occurrences,
        FindingKind::RepeatedLiteral,
        |literal, count| format!("literal {literal:?} is repeated {count} times"),
    ));
    signals.findings.extend(group_occurrences(
        signals.error_patterns,
        options.min_repeated_literal_occurrences,
        FindingKind::RepeatedErrorPattern,
        |_, count| format!("error-handling pattern is repeated {count} times"),
    ));
    signals.findings.extend(group_occurrences(
        signals.data_clumps,
        options.min_data_clump_occurrences,
        FindingKind::DataClump,
        |clump, count| format!("parameter group ({clump}) appears in {count} functions"),
    ));
    signals.findings.extend(group_occurrences(
        signals.test_setups,
        options
            .min_data_clump_occurrences
            .max(MIN_TEST_SETUP_OCCURRENCES),
        FindingKind::TestDuplication,
        |_, count| format!("test setup pattern is repeated {count} times"),
    ));
    signals
        .findings
        .extend(happy_path_test_findings(signals.happy_path_test_files));
    signals
        .findings
        .extend(file_naming_drift_findings(&signals.naming_directories));
    signals
        .findings
        .extend(directory_drift_findings(&signals.directory_files, options));

    Ok(signals.findings)
}

pub(crate) fn collect_raw_structure_metrics(
    files: &[ParsedSourceFile],
) -> Vec<RawStructureFileMetric> {
    files
        .iter()
        .map(|file| {
            let root = file.tree.root_node();
            let is_test = is_test_source(&file.file.path);
            let traversal = StructureTraversal {
                source: &file.file.source,
                family: file.family,
                include_test_structure: true,
            };
            let mut signals = FileSignals::default();
            let ast_signals =
                collect_production_ast_signals(&file.file, root, traversal, &mut signals);
            let path = file.file.display_path.clone();
            RawStructureFileMetric {
                path: path.clone(),
                imports: count_imports(root, file.family),
                public_items: count_public_items(root, traversal),
                is_test,
                functions: ast_signals
                    .functions
                    .into_iter()
                    .map(|function| RawStructureFunctionMetric {
                        path: path.clone(),
                        name: function.name,
                        line: function.line,
                        loc: function.lines,
                        complexity: function.complexity,
                        nesting_depth: function.nesting_depth,
                        parameter_count: function.parameter_count,
                        is_test,
                    })
                    .collect(),
                types: ast_signals
                    .types
                    .into_iter()
                    .map(|type_metric| RawStructureTypeMetric {
                        path: path.clone(),
                        name: type_metric.name,
                        line: type_metric.line,
                        loc: type_metric.lines,
                        member_count: type_metric.members,
                        is_test,
                    })
                    .collect(),
            }
        })
        .collect()
}

pub fn is_supported_structure_source(path: &Path) -> bool {
    adapter_for_path(path).is_some()
}

fn scan_production_file(
    file: &SourceFile,
    family: LanguageFamily,
    root: Node<'_>,
    options: &StructureOptions,
    signals: &mut FileSignals,
) {
    let traversal = StructureTraversal {
        source: &file.source,
        family,
        include_test_structure: options.include_test_structure,
    };

    let ast_signals = collect_production_ast_signals(file, root, traversal, signals);
    scan_function_metrics(file, &ast_signals.functions, options, signals);
    scan_type_metrics(file, &ast_signals.types, options, signals);
    scan_file_metrics(file, root, traversal, options, signals);
    scan_function_proliferation(file, root, &ast_signals.functions, options, signals);
    collect_cross_file_patterns(file, root, traversal, signals);
}

include!("function_findings.rs");

fn collect_cross_file_patterns(
    file: &SourceFile,
    _root: Node<'_>,
    traversal: StructureTraversal<'_>,
    signals: &mut FileSignals,
) {
    collect_directory_concepts(file, traversal.family, signals);
}

fn collect_production_ast_signals(
    file: &SourceFile,
    root: Node<'_>,
    traversal: StructureTraversal<'_>,
    signals: &mut FileSignals,
) -> ProductionAstSignals {
    let mut ast_signals = ProductionAstSignals::default();
    let mut collector = StructureSignalCollector {
        file,
        traversal,
        signals,
    };
    collect_production_ast_signals_from(root, traversal, &mut collector, &mut ast_signals);
    ast_signals
}

fn collect_production_ast_signals_from(
    node: Node<'_>,
    traversal: StructureTraversal<'_>,
    collector: &mut StructureSignalCollector<'_, '_>,
    ast_signals: &mut ProductionAstSignals,
) {
    if should_skip_rust_test_module(node, traversal) {
        return;
    }

    if let Some(parts) = function_parts(node, traversal) {
        let parameter_names = parameter_names(parts.parameters, traversal.source, traversal.family);
        ast_signals.functions.push(FunctionMetric {
            name: parts.name,
            line: node.start_position().row + 1,
            lines: node_line_span(node),
            parameter_count: parameter_names.len(),
            parameter_names,
            complexity: complexity(parts.body, traversal),
            nesting_depth: max_nesting_depth(parts.body, traversal.family, 0),
        });
    }

    if let Some(metric) = type_metric(node, traversal) {
        ast_signals.types.push(metric);
    }

    collector.collect_literal_occurrence(node);
    collector.collect_error_occurrence(node);

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_production_ast_signals_from(child, traversal, collector, ast_signals);
    }
}

include!("syntax_metrics.rs");

mod analysis;
mod parameters;

use analysis::*;
use parameters::*;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn source_file(path: &str, source: &str) -> SourceFile {
        SourceFile {
            path: PathBuf::from(path),
            display_path: path.to_string(),
            source: source.into(),
        }
    }

    #[test]
    fn collects_bash_function_metrics() -> Result<()> {
        let source = r#"
deploy_app() {
  for target in "$@"; do
    if [ -n "$target" ]; then
      case "$target" in
        prod) echo "deploy prod" ;;
        *) echo "deploy other" ;;
      esac
    fi
  done
}
"#;
        let parsed = parse_source_files(&[source_file("scripts/deploy.sh", source)])?;
        let metrics = collect_raw_structure_metrics(&parsed);

        assert_eq!(metrics.len(), 1);
        let function = &metrics[0].functions[0];
        assert_eq!(function.name, "deploy_app");
        assert_eq!(function.parameter_count, 0);
        assert!(function.complexity >= 5, "{function:?}");
        assert!(function.nesting_depth >= 3, "{function:?}");
        Ok(())
    }

    #[test]
    fn collects_powershell_signature_and_param_block_metrics() -> Result<()> {
        let source = r#"
function Invoke-Deploy($Path, [switch]$Force) {
  if ($Force) {
    foreach ($item in Get-ChildItem $Path) {
      Write-Output $item
    }
  }
}

function Test-Release {
  param([string]$Name, $Count = $DefaultCount)
  while ($Count -gt 0) {
    $Count--
  }
}
"#;
        let parsed = parse_source_files(&[source_file("scripts/deploy.ps1", source)])?;
        let metrics = collect_raw_structure_metrics(&parsed);
        let functions = &metrics[0].functions;

        assert_eq!(functions.len(), 2, "{functions:?}");
        assert_eq!(functions[0].name, "Invoke-Deploy");
        assert_eq!(functions[0].parameter_count, 2);
        assert!(functions[0].complexity >= 3, "{:?}", functions[0]);
        assert!(functions[0].nesting_depth >= 2, "{:?}", functions[0]);
        assert_eq!(functions[1].name, "Test-Release");
        assert_eq!(functions[1].parameter_count, 2);
        Ok(())
    }
}
