fn internal_edge_count(graph: &DependencyGraph, paths: &[String]) -> usize {
    let members = paths.iter().map(String::as_str).collect::<BTreeSet<_>>();
    paths
        .iter()
        .filter_map(|path| graph.nodes.get(path))
        .map(|node| {
            node.edges
                .iter()
                .filter(|target| members.contains(target.as_str()))
                .count()
        })
        .sum()
}

fn edge_density_percent(edge_count: usize, node_count: usize) -> usize {
    let possible_edges = node_count.saturating_mul(node_count.saturating_sub(1));
    if possible_edges == 0 {
        return 0;
    }

    ((edge_count * 100) + (possible_edges / 2)) / possible_edges
}

fn fan_in_counts(graph: &DependencyGraph) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::<String, usize>::new();
    for node in graph.nodes.values() {
        for target in &node.edges {
            *counts.entry(target.clone()).or_default() += 1;
        }
    }
    counts
}

fn reverse_edges(graph: &DependencyGraph) -> BTreeMap<String, BTreeSet<String>> {
    let mut reverse = graph
        .nodes
        .keys()
        .map(|path| (path.clone(), BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();

    for node in graph.nodes.values() {
        for target in &node.edges {
            reverse
                .entry(target.clone())
                .or_default()
                .insert(node.path.clone());
        }
    }

    reverse
}

fn reachable_count(graph: &DependencyGraph, start: &str) -> usize {
    reachable_paths(start, |path| {
        graph
            .nodes
            .get(path)
            .map(|node| node.edges.iter().cloned().collect())
            .unwrap_or_default()
    })
}

fn reverse_reachable_count(
    reverse_edges: &BTreeMap<String, BTreeSet<String>>,
    start: &str,
) -> usize {
    reachable_paths(start, |path| {
        reverse_edges
            .get(path)
            .map(|edges| edges.iter().cloned().collect())
            .unwrap_or_default()
    })
}

fn reachable_paths(start: &str, mut edges_for: impl FnMut(&str) -> Vec<String>) -> usize {
    let mut seen = BTreeSet::<String>::new();
    let mut stack = edges_for(start);

    while let Some(path) = stack.pop() {
        if !seen.insert(path.clone()) {
            continue;
        }

        for target in edges_for(&path) {
            if target != start && !seen.contains(&target) {
                stack.push(target);
            }
        }
    }

    seen.remove(start);
    seen.len()
}

fn dependency_depths(graph: &DependencyGraph) -> BTreeMap<String, usize> {
    let components = strongly_connected_components(graph);
    let mut component_by_path = BTreeMap::<String, usize>::new();
    for (component_index, component) in components.iter().enumerate() {
        for path in component {
            component_by_path.insert(path.clone(), component_index);
        }
    }

    let mut component_edges = vec![BTreeSet::<usize>::new(); components.len()];
    for node in graph.nodes.values() {
        let Some(&source_component) = component_by_path.get(&node.path) else {
            continue;
        };
        for target in &node.edges {
            let Some(&target_component) = component_by_path.get(target) else {
                continue;
            };
            if source_component != target_component {
                component_edges[source_component].insert(target_component);
            }
        }
    }

    let mut memo = vec![None; components.len()];
    for component_index in 0..components.len() {
        component_dependency_depth(component_index, &component_edges, &mut memo);
    }

    component_by_path
        .into_iter()
        .map(|(path, component_index)| (path, memo[component_index].unwrap_or(0)))
        .collect()
}

fn component_dependency_depth(
    component: usize,
    edges: &[BTreeSet<usize>],
    memo: &mut [Option<usize>],
) -> usize {
    if let Some(depth) = memo[component] {
        return depth;
    }

    let depth = edges[component]
        .iter()
        .map(|target| 1 + component_dependency_depth(*target, edges, memo))
        .max()
        .unwrap_or(0);
    memo[component] = Some(depth);
    depth
}

fn instability_percent(fan_in: usize, fan_out: usize) -> usize {
    let total = fan_in + fan_out;
    if total == 0 {
        return 0;
    }

    ((fan_out * 100) + (total / 2)) / total
}

fn is_hub_degree(degree: usize, baseline: usize) -> bool {
    degree >= MIN_HUB_DEGREE && degree >= baseline.saturating_mul(HUB_OUTLIER_MULTIPLIER).max(1)
}

fn percentile(values: &[usize], percentile: f64) -> usize {
    if values.is_empty() {
        return 0;
    }

    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let index = ((sorted.len() - 1) as f64 * percentile).ceil() as usize;
    sorted[index.min(sorted.len() - 1)]
}

fn strongly_connected_components(graph: &DependencyGraph) -> Vec<Vec<String>> {
    Tarjan::new(graph).components()
}

struct Tarjan<'a> {
    graph: &'a DependencyGraph,
    index: usize,
    stack: Vec<String>,
    indices: BTreeMap<String, usize>,
    lowlinks: BTreeMap<String, usize>,
    on_stack: BTreeSet<String>,
    components: Vec<Vec<String>>,
}

impl<'a> Tarjan<'a> {
    fn new(graph: &'a DependencyGraph) -> Self {
        Self {
            graph,
            index: 0,
            stack: Vec::new(),
            indices: BTreeMap::new(),
            lowlinks: BTreeMap::new(),
            on_stack: BTreeSet::new(),
            components: Vec::new(),
        }
    }

    fn components(mut self) -> Vec<Vec<String>> {
        for path in self.graph.nodes.keys() {
            if !self.indices.contains_key(path) {
                self.connect(path);
            }
        }
        self.components
    }

    fn connect(&mut self, path: &str) {
        self.push_path(path);
        for target in self.edges_for(path) {
            self.visit_edge(path, &target);
        }
        self.emit_component_if_root(path);
    }

    fn push_path(&mut self, path: &str) {
        self.indices.insert(path.to_string(), self.index);
        self.lowlinks.insert(path.to_string(), self.index);
        self.index += 1;
        self.stack.push(path.to_string());
        self.on_stack.insert(path.to_string());
    }

    fn edges_for(&self, path: &str) -> Vec<String> {
        self.graph
            .nodes
            .get(path)
            .map(|node| node.edges.iter().cloned().collect())
            .unwrap_or_default()
    }

    fn visit_edge(&mut self, path: &str, target: &str) {
        if !self.indices.contains_key(target) {
            self.connect(target);
            self.merge_child_lowlink(path, target);
        } else if self.on_stack.contains(target) {
            self.merge_stack_index(path, target);
        }
    }

    fn merge_child_lowlink(&mut self, path: &str, target: &str) {
        let target_lowlink = self.lowlinks[target];
        let path_lowlink = self.lowlinks.get_mut(path).expect("path should be known");
        *path_lowlink = (*path_lowlink).min(target_lowlink);
    }

    fn merge_stack_index(&mut self, path: &str, target: &str) {
        let target_index = self.indices[target];
        let path_lowlink = self.lowlinks.get_mut(path).expect("path should be known");
        *path_lowlink = (*path_lowlink).min(target_index);
    }

    fn emit_component_if_root(&mut self, path: &str) {
        if self.indices[path] == self.lowlinks[path] {
            let component = self.pop_component(path);
            self.components.push(component);
        }
    }

    fn pop_component(&mut self, root: &str) -> Vec<String> {
        let mut component = Vec::new();
        while let Some(member) = self.stack.pop() {
            self.on_stack.remove(&member);
            let is_root = member == root;
            component.push(member);
            if is_root {
                break;
            }
        }
        component
    }
}
