use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct DataFlowConfig {
    pub max_function_hops: usize,
    pub max_path_steps: usize,
    pub max_module_hops: usize,
    pub max_paths_per_source: usize,
    pub max_sinks_per_source: usize,
    pub work_budget: usize,
    pub min_function_hops: usize,
    pub min_module_hops: usize,
    pub min_relay_percent: usize,
    pub min_sinks: usize,
    pub min_modules: usize,
    pub boundaries: Vec<DataFlowBoundaryConfig>,
}

impl Default for DataFlowConfig {
    fn default() -> Self {
        Self {
            max_function_hops: 8,
            max_path_steps: 24,
            max_module_hops: 8,
            max_paths_per_source: 100,
            max_sinks_per_source: 100,
            work_budget: 100_000,
            min_function_hops: 4,
            min_module_hops: 2,
            min_relay_percent: 90,
            min_sinks: 4,
            min_modules: 3,
            boundaries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct DataFlowBoundaryConfig {
    pub name: String,
    pub protected_paths: Vec<String>,
    pub adapter_paths: Vec<String>,
    pub sink_symbols: Vec<String>,
    #[serde(default)]
    pub exempt_paths: Vec<String>,
}
