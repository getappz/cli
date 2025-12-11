// This file contains code adapted from mise (https://github.com/jdx/mise)
// Original source: src/task/deps.rs
// License: MIT (Copyright (c) 2025 Jeff Dickey)
// See: C:\Users\shiva\code-ref\mise\LICENSE

use crate::registry::TaskRegistry;
use miette::{miette, Result};
use petgraph::{algo, graph::DiGraph, Direction};
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;

/// Builds a dependency graph from the registry for the given target task.
/// Includes all reachable tasks, dependencies, hooks, and wait_for relationships.
pub fn build_task_graph(
    registry: &TaskRegistry,
    target: &str,
) -> Result<(
    DiGraph<String, ()>,
    HashMap<String, petgraph::graph::NodeIndex>,
)> {
    // Collect reachable nodes (deps + hooks + wait_for if they exist)
    let mut nodes: HashSet<String> = HashSet::new();
    let mut stack: Vec<String> = vec![target.to_string()];

    while let Some(n) = stack.pop() {
        if !nodes.insert(n.clone()) {
            continue;
        }
        let t = registry
            .get(&n)
            .ok_or_else(|| miette!("Unknown task '{}'", n))?;

        // before hooks
        if let Some(hs) = registry.hooks.before.get(&t.name) {
            for h in hs {
                stack.push(h.clone());
            }
        }

        // deps
        for dep in &t.deps {
            stack.push(dep.clone());
        }

        // wait_for tasks (soft dependencies - only include if they exist in registry)
        for w in &t.wait_for {
            if registry.get(w).is_some() {
                stack.push(w.clone());
            }
        }

        // after hooks
        if let Some(hs) = registry.hooks.after.get(&t.name) {
            for h in hs {
                stack.push(h.clone());
            }
        }
    }

    // Build petgraph DiGraph from the reachable nodes
    let mut graph = DiGraph::<String, ()>::new();
    let mut node_map: HashMap<String, petgraph::graph::NodeIndex> = HashMap::new();

    // Add all nodes to the graph
    for node_name in &nodes {
        let idx = graph.add_node(node_name.clone());
        node_map.insert(node_name.clone(), idx);
    }

    // Add edges: deps, wait_for, before hooks, after hooks
    for node_name in &nodes {
        let t = registry.get(node_name).unwrap();
        let target_idx = node_map[node_name];

        // edges from before hooks -> task
        if let Some(hs) = registry.hooks.before.get(&t.name) {
            for b in hs {
                if let Some(&hook_idx) = node_map.get(b) {
                    graph.add_edge(hook_idx, target_idx, ());
                }
            }
        }

        // edges from deps -> task
        for d in &t.deps {
            if let Some(&dep_idx) = node_map.get(d) {
                graph.add_edge(dep_idx, target_idx, ());
            }
        }

        // edges from wait_for -> task (soft dependencies - only if in nodes)
        for w in &t.wait_for {
            if let Some(&wait_idx) = node_map.get(w) {
                graph.add_edge(wait_idx, target_idx, ());
            }
        }

        // edges from task -> after hooks
        if let Some(hs) = registry.hooks.after.get(&t.name) {
            for a in hs {
                if let Some(&after_idx) = node_map.get(a) {
                    graph.add_edge(target_idx, after_idx, ());
                }
            }
        }
    }

    Ok((graph, node_map))
}

/// Builds a dependency graph from a plan (ordered list of task names).
/// Used for wave computation where we only need tasks in the execution plan.
pub fn build_graph_from_plan(
    registry: &TaskRegistry,
    plan: &[String],
) -> (
    DiGraph<String, ()>,
    HashMap<String, petgraph::graph::NodeIndex>,
) {
    let mut graph = DiGraph::<String, ()>::new();
    let mut node_map: HashMap<String, petgraph::graph::NodeIndex> = HashMap::new();

    // Add all nodes in plan to the graph
    for task_name in plan {
        let idx = graph.add_node(task_name.clone());
        node_map.insert(task_name.clone(), idx);
    }

    // Add edges based on dependencies
    for task_name in plan {
        if let Some(task) = registry.get(task_name) {
            let target_idx = node_map[task_name];

            // Add explicit dependencies
            for dep in &task.deps {
                if let Some(&dep_idx) = node_map.get(dep) {
                    graph.add_edge(dep_idx, target_idx, ());
                }
            }

            // Add wait_for as dependencies (soft dependencies - only if in execution plan)
            // From mise: wait_for tasks are only included if they're in tasks_to_run
            // This matches mise's behavior where wait_for is conditional
            for w in &task.wait_for {
                // Only add edge if wait_for task is in the execution plan (node_map)
                // If not in plan, skip it (soft dependency - optional)
                if let Some(&wait_idx) = node_map.get(w) {
                    graph.add_edge(wait_idx, target_idx, ());
                }
            }

            // Add before hooks as dependencies
            if let Some(before_hooks) = registry.hooks.before.get(&task.name) {
                for hook in before_hooks {
                    if let Some(&hook_idx) = node_map.get(hook) {
                        graph.add_edge(hook_idx, target_idx, ());
                    }
                }
            }
        }
    }

    // Also track reverse dependencies: after hooks depend on their target task
    for task_name in plan {
        if let Some(task) = registry.get(task_name) {
            if let Some(after_hooks) = registry.hooks.after.get(&task.name) {
                let task_idx = node_map[task_name];
                for hook in after_hooks {
                    if let Some(&hook_idx) = node_map.get(hook) {
                        graph.add_edge(task_idx, hook_idx, ());
                    }
                }
            }
        }
    }

    (graph, node_map)
}

/// Performs topological sort on the graph and returns ordered task names.
/// Returns error if cycle is detected.
pub fn topological_sort(graph: &DiGraph<String, ()>) -> Result<Vec<String>> {
    match algo::toposort(graph, None) {
        Ok(indices) => {
            let order: Vec<String> = indices.iter().map(|&idx| graph[idx].clone()).collect();
            Ok(order)
        }
        Err(_cycle) => Err(miette!("Cycle detected in task graph")),
    }
}

/// Groups tasks into waves where tasks in the same wave can run in parallel.
/// Each wave contains tasks whose dependencies (in the previous waves) are all satisfied.
///
/// DEPRECATED: This function is kept for backward compatibility but is no longer used
/// in the streaming execution model. Use `Deps` struct instead.
#[deprecated(note = "Use Deps struct for streaming execution")]
pub fn compute_waves(graph: &DiGraph<String, ()>) -> Vec<Vec<String>> {
    let mut waves = Vec::new();
    let mut graph_mut = graph.clone(); // Clone to allow mutation during iteration

    while graph_mut.node_count() > 0 {
        // Find all nodes with no incoming edges (tasks ready to run)
        let ready_indices: Vec<_> = graph_mut
            .node_indices()
            .filter(|&idx| {
                graph_mut
                    .neighbors_directed(idx, Direction::Incoming)
                    .next()
                    .is_none()
            })
            .collect();

        if ready_indices.is_empty() {
            // Should not happen if plan is valid, but handle gracefully
            // Add remaining nodes as a final wave
            let remaining: Vec<String> = graph_mut.node_weights().cloned().collect();
            if !remaining.is_empty() {
                waves.push(remaining);
            }
            break;
        }

        // Extract task names for this wave
        let ready_tasks: Vec<String> = ready_indices
            .iter()
            .map(|&idx| graph_mut[idx].clone())
            .collect();

        waves.push(ready_tasks.clone());

        // Remove ready tasks from graph (along with their outgoing edges)
        // Note: Removing nodes invalidates indices, so we remove in reverse order
        for idx in ready_indices.iter().rev() {
            graph_mut.remove_node(*idx);
        }
    }

    waves
}

/// Manages a dependency graph of tasks using mise's streaming execution model.
/// Tasks are emitted as soon as their dependencies are satisfied, rather than
/// waiting for entire waves to complete.
///
/// Adapted from mise's Deps struct to work with saasctl's task name-based representation.
#[derive(Debug)]
pub struct Deps {
    pub graph: DiGraph<String, ()>,
    sent: HashSet<String>, // tasks that have already been sent to avoid duplicates
    removed: HashSet<String>, // tasks that have already finished to detect infinite loops
    tx: mpsc::UnboundedSender<Option<String>>,
}

impl Deps {
    /// Create a new Deps instance from a graph and node map.
    /// The graph should already contain all tasks and their dependencies.
    pub fn new(graph: DiGraph<String, ()>) -> Self {
        let (tx, _) = mpsc::unbounded_channel();
        Self {
            graph,
            sent: HashSet::new(),
            removed: HashSet::new(),
            tx,
        }
    }

    /// Main method to emit tasks that no longer have dependencies being waited on.
    /// Called automatically when tasks are removed from the graph.
    fn emit_leaves(&mut self) {
        let leaves = leaves(&self.graph);
        let leaves_is_empty = leaves.is_empty();

        for task_name in leaves {
            if self.sent.insert(task_name.clone()) {
                // trace!("Scheduling task {}", task_name);
                if let Err(_e) = self.tx.send(Some(task_name)) {
                    // trace!("Error sending task: {_e:?}");
                }
            }
        }

        if self.is_empty() {
            // trace!("All tasks finished");
            if let Err(_e) = self.tx.send(None) {
                // trace!("Error closing task stream: {_e:?}");
            }
        } else if leaves_is_empty && self.sent.len() == self.removed.len() {
            // Infinite loop detection: no leaves available but graph not empty
            // and we've sent as many tasks as we've removed
            let remaining: Vec<String> = self.graph.node_weights().cloned().collect();
            panic!(
                "Infinite loop detected: all tasks are finished but the graph isn't empty. Remaining tasks: {:?}",
                remaining
            );
        }
    }

    /// Subscribe to the stream of ready tasks.
    /// Returns a channel receiver that will emit tasks as they become ready.
    /// The channel emits `Some(task_name)` for ready tasks and `None` when all tasks are done.
    pub fn subscribe(&mut self) -> mpsc::UnboundedReceiver<Option<String>> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.tx = tx;
        self.emit_leaves();
        rx
    }

    /// Check if the dependency graph is empty (all tasks completed).
    pub fn is_empty(&self) -> bool {
        self.graph.node_count() == 0
    }

    /// Remove a completed task from the graph and emit any newly-ready tasks.
    /// This should be called when a task finishes execution.
    pub fn remove(&mut self, task_name: &str) {
        if let Some(idx) = self.node_idx(task_name) {
            self.graph.remove_node(idx);
            self.removed.insert(task_name.to_string());
            self.emit_leaves();
        }
    }

    /// Find the node index for a given task name.
    fn node_idx(&self, task_name: &str) -> Option<petgraph::graph::NodeIndex> {
        self.graph
            .node_indices()
            .find(|&idx| self.graph[idx] == task_name)
    }

    /// Get an iterator over all remaining tasks in the graph.
    pub fn all(&self) -> impl Iterator<Item = &String> {
        self.graph.node_indices().map(|idx| &self.graph[idx])
    }
}

/// Find all leaf nodes (tasks with no incoming edges/dependencies) in the graph.
/// These are tasks that are ready to execute.
fn leaves(graph: &DiGraph<String, ()>) -> Vec<String> {
    graph
        .externals(Direction::Incoming)
        .map(|idx| graph[idx].clone())
        .collect()
}
