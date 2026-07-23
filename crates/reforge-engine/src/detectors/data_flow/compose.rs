use std::collections::{BTreeMap, VecDeque};

use super::model::{CallTransition, FlowGraph, NodeId};

#[derive(Debug, Clone)]
pub(super) struct ExactPath {
    pub source: NodeId,
    pub sink: NodeId,
    pub edges: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SearchState {
    node: NodeId,
    stack: Vec<String>,
}

enum Advance {
    Next(SearchState),
    Truncated,
    Invalid,
}

pub(super) fn shortest_exact_path(
    graph: &FlowGraph,
    source: NodeId,
    sink: NodeId,
    max_hops: usize,
) -> (Option<ExactPath>, usize) {
    let mut outgoing = BTreeMap::<NodeId, Vec<usize>>::new();
    for (index, edge) in graph.edges.iter().enumerate() {
        outgoing.entry(edge.from).or_default().push(index);
    }
    let start = SearchState {
        node: source,
        stack: Vec::new(),
    };
    let mut queue = VecDeque::from([(start.clone(), Vec::new())]);
    let mut distances = BTreeMap::from([(start, 0usize)]);
    let mut truncated = 0;

    while let Some((state, path)) = queue.pop_front() {
        if state.node == sink {
            return (
                Some(ExactPath {
                    source,
                    sink,
                    edges: path,
                }),
                truncated,
            );
        }
        for edge_index in outgoing.get(&state.node).into_iter().flatten().copied() {
            let edge = &graph.edges[edge_index];
            let next = match advance_state(&state, edge, max_hops) {
                Advance::Next(next) => next,
                Advance::Truncated => {
                    truncated += 1;
                    continue;
                }
                Advance::Invalid => continue,
            };
            let distance = path.len() + 1;
            if distances.get(&next).is_some_and(|known| *known <= distance) {
                continue;
            }
            distances.insert(next.clone(), distance);
            let mut next_path = path.clone();
            next_path.push(edge_index);
            queue.push_back((next, next_path));
        }
    }
    (None, truncated)
}

fn advance_state(state: &SearchState, edge: &super::model::FlowEdge, max_hops: usize) -> Advance {
    let mut next = SearchState {
        node: edge.to,
        stack: state.stack.clone(),
    };
    match edge.transition {
        CallTransition::None => {}
        CallTransition::Enter if next.stack.len() >= max_hops => return Advance::Truncated,
        CallTransition::Enter => next.stack.push(edge.call_site.clone().unwrap_or_default()),
        CallTransition::Return => {
            let Some(call_site) = edge.call_site.as_deref() else {
                return Advance::Invalid;
            };
            if next.stack.last().map(String::as_str) != Some(call_site) {
                return Advance::Invalid;
            }
            next.stack.pop();
        }
    }
    Advance::Next(next)
}
