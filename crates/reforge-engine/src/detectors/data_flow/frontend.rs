use std::path::Path;

use crate::detectors::similarity::ParsedSourceFile;
use crate::lang::LanguageFamily;

use super::model::FlowGraph;

pub(super) trait SemanticFrontend {
    fn extend_graph(&self, root: &Path, files: &[ParsedSourceFile], graph: &mut FlowGraph);
}

pub(super) struct RustFrontend;
pub(super) struct JavaScriptTypeScriptFrontend;
pub(super) struct PythonFrontend;

impl SemanticFrontend for RustFrontend {
    fn extend_graph(&self, root: &Path, files: &[ParsedSourceFile], graph: &mut FlowGraph) {
        debug_assert!(graph.nodes.is_empty());
        *graph = super::rust::build_graph(root, files);
    }
}

impl SemanticFrontend for JavaScriptTypeScriptFrontend {
    fn extend_graph(&self, root: &Path, files: &[ParsedSourceFile], graph: &mut FlowGraph) {
        super::dynamic::extend_graph(root, files, graph, LanguageFamily::JavaScriptTypeScript);
    }
}

impl SemanticFrontend for PythonFrontend {
    fn extend_graph(&self, root: &Path, files: &[ParsedSourceFile], graph: &mut FlowGraph) {
        super::dynamic::extend_graph(root, files, graph, LanguageFamily::Python);
    }
}
