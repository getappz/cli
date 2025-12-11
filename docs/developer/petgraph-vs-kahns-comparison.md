# Petgraph vs Kahn's Algorithm: Comparison

## Key Insight

**petgraph doesn't replace Kahn's algorithm** - it **uses Kahn's algorithm internally** for topological sorting, but provides better abstractions, data structures, and additional graph utilities.

## Current Implementation (Manual Kahn's Algorithm)

### What We're Doing Now

Our current code in `runner.rs` manually implements Kahn's algorithm:

```rust
// 1. Manual graph building with HashMap/HashSet
let mut indeg: HashMap<String, usize> = HashMap::new();
let mut adj: HashMap<String, Vec<String>> = HashMap::new();

// 2. Manual indegree calculation
for d in &t.deps {
    adj.get_mut(d).unwrap().push(t.name.clone());
    *indeg.get_mut(&t.name).unwrap() += 1;
}

// 3. Manual Kahn's queue
let mut q: VecDeque<String> = VecDeque::new();
for (n, &deg) in indeg.iter() {
    if deg == 0 {
        q.push_back(n.clone());
    }
}

// 4. Manual topological sort
while let Some(u) = q.pop_front() {
    order.push(u.clone());
    for v in adj.get(&u).unwrap().iter() {
        let d = indeg.get_mut(v).unwrap();
        *d -= 1;
        if *d == 0 {
            q.push_back(v.clone());
        }
    }
}

// 5. Manual cycle detection
if order.len() != indeg.len() {
    return Err(miette!("Cycle detected"));
}
```

### Current Approach Characteristics

- ✅ **Pros**:
  - No external dependencies
  - Direct control over the algorithm
  - Simple for basic use cases
  - Easy to understand the exact logic

- ❌ **Cons**:
  - ~150 lines of manual graph code
  - Manual cycle detection (not as robust)
  - Manual graph operations (building, traversing)
  - Harder to extend with new graph features
  - More error-prone (manual index management)
  - Duplicated logic for `plan()` and `group_into_waves()`

## Petgraph Approach

### What Petgraph Provides

petgraph is a Rust graph library that includes:

1. **Graph Data Structures**: `DiGraph`, `UnGraph`, etc.
2. **Graph Algorithms**: Including `toposort()` which uses Kahn's algorithm internally
3. **Graph Utilities**: Cycle detection, shortest paths, etc.

### How It Would Look

```rust
use petgraph::{DiGraph, algo};

// 1. Build graph (cleaner)
let mut graph = DiGraph::<String, ()>::new();
let mut node_map = HashMap::new();

// Add nodes
for task_name in &nodes {
    let idx = graph.add_node(task_name.clone());
    node_map.insert(task_name.clone(), idx);
}

// Add edges
graph.add_edge(node_map[&dep], node_map[&task], ());

// 2. Topological sort (one line, uses Kahn's internally)
match algo::toposort(&graph, None) {
    Ok(indices) => {
        let order: Vec<String> = indices
            .iter()
            .map(|idx| graph[*idx].clone())
            .collect();
        Ok(order)
    }
    Err(cycle) => {
        // Cycle detected automatically
        Err(miette!("Cycle detected in task graph"))
    }
}

// 3. Cycle detection (built-in)
if algo::is_cyclic_directed(&graph) {
    return Err(miette!("Cycle detected"));
}
```

### Petgraph Approach Characteristics

- ✅ **Pros**:
  - **Less code**: ~50-80 lines vs ~150 lines
  - **Built-in cycle detection**: More robust (`is_cyclic_directed()`)
  - **Well-tested library**: Used by many Rust projects
  - **Better data structures**: Optimized graph representation
  - **More graph operations**: Easy to add shortest paths, DFS, etc.
  - **Cleaner abstractions**: Graph is first-class data structure
  - **Better performance**: For large graphs (optimized internals)
  - **Used by mise and moon**: Proven in similar projects

- ❌ **Cons**:
  - **Additional dependency**: +1 crate (but widely used)
  - **Slight abstraction**: Need to map NodeIndex ↔ task names
  - **Learning curve**: Need to understand petgraph API

## Algorithm Comparison

### Both Use Kahn's Algorithm

| Aspect | Manual Implementation | Petgraph |
|--------|----------------------|----------|
| **Algorithm** | Kahn's algorithm | Kahn's algorithm (in `toposort()`) |
| **Complexity** | O(V + E) | O(V + E) |
| **Correctness** | Same (if implemented correctly) | Same (well-tested) |
| **Code Lines** | ~150 lines | ~50-80 lines |
| **Maintenance** | Manual maintenance | Library maintained |

### Key Difference: Abstraction Level

**Manual**: You implement Kahn's algorithm step-by-step  
**Petgraph**: You use `algo::toposort()` which implements Kahn's internally

## Real-World Usage Examples

### Mise's Implementation

Looking at mise's `src/task/deps.rs`:

```rust
use petgraph::graph::DiGraph;

pub struct Deps {
    pub graph: DiGraph<Task, ()>,
    // ...
}

// Uses petgraph's graph structure for dependency management
// Leverages built-in operations like externals(), neighbors(), etc.
```

**Key insight**: Mise uses petgraph not just for toposort, but for:
- Graph manipulation (add/remove nodes)
- Finding leaves (`externals(Direction::Outgoing)`)
- Graph traversal operations

### Moon's Implementation

Looking at moon's `action-graph.rs`:

```rust
match petgraph::algo::toposort(&self.graph, None) {
    Ok(mut indices) => {
        indices.reverse();
        Ok(indices)
    }
    Err(cycle) => {
        // Cycle detected automatically
        Err(ActionGraphError::CycleDetected(...))
    }
}
```

## Performance Comparison

### Time Complexity
- **Both**: O(V + E) - same asymptotic complexity
- **Manual**: Potentially faster for very small graphs (no overhead)
- **Petgraph**: Better for larger graphs (optimized data structures)

### Space Complexity
- **Both**: O(V + E)
- **Petgraph**: Slightly more efficient (compact representation)

### Benchmark Notes
- For typical task graphs (10-100 tasks): Negligible difference
- For large graphs (1000+ tasks): Petgraph may be faster
- Memory usage: Petgraph is more efficient

## Code Quality Comparison

### Maintainability

**Manual Implementation**:
```rust
// Need to manually track:
let mut indeg: HashMap<String, usize> = HashMap::new();
let mut adj: HashMap<String, Vec<String>> = HashMap::new();
// ... 100+ more lines of manual graph logic
```

**Petgraph**:
```rust
let graph = DiGraph::new();
// Graph operations are built-in
// Less code, less bugs
```

### Extensibility

**Manual**: Want to add shortest path? Implement Dijkstra's manually  
**Petgraph**: `algo::dijkstra()` already exists

**Manual**: Want to find all paths? Implement DFS manually  
**Petgraph**: `algo::dfs()` already exists

## Recommendation

### ✅ **Use Petgraph** Because:

1. **Industry Standard**: Both mise and moon (similar projects) use petgraph
2. **Less Code**: Significantly reduces complexity
3. **Better Maintainability**: Less manual graph code = fewer bugs
4. **Future-Proof**: Easy to add graph features later
5. **Robust**: Well-tested library with cycle detection
6. **Performance**: Optimized for Rust

### ⚠️ **Considerations**:

1. **Dependency**: Adds one crate (petgraph is widely used, low risk)
2. **Abstraction**: Need to map between NodeIndex and task names (simple wrapper)

## Migration Impact

### What Changes
- ✅ **Internal implementation only**: Same public API
- ✅ **Same behavior**: Identical output
- ✅ **Better code quality**: Cleaner, more maintainable

### What Stays the Same
- ✅ **Public API**: `plan()`, `group_into_waves()`, `invoke()` unchanged
- ✅ **Error messages**: Can match exactly
- ✅ **Execution order**: Same topological ordering
- ✅ **Features**: All features (deps, hooks, wait_for) work the same

## Conclusion

**Petgraph is better than manual Kahn's algorithm** not because it uses a different algorithm, but because:

1. It provides the same algorithm with better abstractions
2. Less code to maintain
3. More features available
4. Better tested
5. Industry standard (mise, moon use it)

The migration is **low-risk** because:
- It's internal refactoring only
- Same algorithm (Kahn's) under the hood
- Well-tested library
- Can verify identical behavior with tests

**Verdict**: ✅ **Adopt petgraph** - it's the right tool for the job.

