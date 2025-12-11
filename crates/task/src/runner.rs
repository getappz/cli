use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use miette::miette;
use num_cpus;
use tokio::time::{sleep, timeout};
use tokio_util::sync::CancellationToken;

use crate::{
    context::Context,
    deps::{build_graph_from_plan, build_task_graph, compute_waves, topological_sort, Deps},
    error::TaskResult,
    registry::TaskRegistry,
    scheduler::Scheduler,
    source_tracker::SourceTracker,
    task_executor::TaskExecutor,
    types::AsyncTaskFn,
};

/// Represents the execution state of a task
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// Task has not been executed yet
    Pending,
    /// Task executed successfully
    Passed,
    /// Task execution failed
    Failed,
    /// Task was skipped (due to conditions or failed dependencies)
    Skipped,
}

impl TaskState {
    /// Returns true if the task completed successfully (Passed or Skipped)
    pub fn is_complete(&self) -> bool {
        matches!(self, TaskState::Passed | TaskState::Skipped)
    }
}

/// Represents the type of operation being tracked
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationType {
    /// Task execution
    TaskExecution { task_name: String },
    /// Dependency validation
    DependencyCheck {
        task_name: String,
        dependencies: Vec<String>,
    },
    /// Condition evaluation
    ConditionCheck { task_name: String },
    /// Task planning (topological sort)
    TaskPlanning { target: String },
    /// Wave execution
    WaveExecution {
        wave_index: usize,
        tasks: Vec<String>,
    },
}

/// Represents a tracked operation with timing and status
#[derive(Debug, Clone)]
pub struct Operation {
    /// Type of operation
    pub op_type: OperationType,
    /// Duration of the operation
    pub duration: Option<Duration>,
    /// Status of the operation
    pub status: OperationStatus,
    /// Optional error message if the operation failed
    pub error: Option<String>,
    /// Start time (for internal tracking)
    pub(crate) start_time: Option<Instant>,
}

/// Status of an operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationStatus {
    /// Operation is in progress
    Running,
    /// Operation completed successfully
    Passed,
    /// Operation failed
    Failed,
    /// Operation was skipped
    Skipped,
}

impl Operation {
    /// Create a new operation with the given type
    pub fn new(op_type: OperationType) -> Self {
        Self {
            op_type,
            duration: None,
            status: OperationStatus::Running,
            error: None,
            start_time: Some(Instant::now()),
        }
    }

    /// Mark the operation as finished with the given status
    pub fn finish(&mut self, status: OperationStatus) {
        self.status = status;
        if let Some(start) = self.start_time {
            self.duration = Some(start.elapsed());
        }
    }

    /// Mark the operation as finished with an error
    pub fn finish_with_error(&mut self, error: impl Into<String>) {
        self.status = OperationStatus::Failed;
        self.error = Some(error.into());
        if let Some(start) = self.start_time {
            self.duration = Some(start.elapsed());
        }
    }

    /// Get a human-readable description of the operation
    pub fn description(&self) -> String {
        match &self.op_type {
            OperationType::TaskExecution { task_name } => {
                format!("Executing task '{}'", task_name)
            }
            OperationType::DependencyCheck {
                task_name,
                dependencies,
            } => {
                if dependencies.is_empty() {
                    format!("Checking dependencies for task '{}'", task_name)
                } else {
                    format!(
                        "Checking dependencies for task '{}': {}",
                        task_name,
                        dependencies.join(", ")
                    )
                }
            }
            OperationType::ConditionCheck { task_name } => {
                format!("Evaluating conditions for task '{}'", task_name)
            }
            OperationType::TaskPlanning { target } => {
                format!("Planning execution order for target '{}'", target)
            }
            OperationType::WaveExecution { wave_index, tasks } => {
                format!(
                    "Executing wave {} with {} task(s): {}",
                    wave_index + 1,
                    tasks.len(),
                    tasks.join(", ")
                )
            }
        }
    }
}

pub struct Runner<'a> {
    registry: &'a TaskRegistry,
    ran_once: Arc<Mutex<HashSet<String>>>,
    /// Tracks the execution state of each task
    task_states: Arc<Mutex<HashMap<String, TaskState>>>,
    /// Tracks operations for debugging
    operations: Arc<Mutex<Vec<Operation>>>,
    /// When true, print minimal live progress to stdout
    verbose: bool,
    /// Maximum number of concurrent tasks (None = unlimited, default = num_cpus max 8)
    jobs: Option<usize>,
    /// Continue executing even if a task fails
    continue_on_error: bool,
}

/// Execute a task with timeout support, following moonrepo's pattern
/// Uses CancellationToken and tokio::select! for true cancellation
async fn execute_task_with_timeout(
    task_name: &str,
    timeout_secs: Option<u64>,
    task_fn: AsyncTaskFn,
    ctx: Arc<Context>,
) -> TaskResult {
    let timeout_token = CancellationToken::new();
    let cancel_clone = timeout_token.clone();

    // Spawn timeout monitor if requested (matches moonrepo's monitor_timeout pattern)
    let timeout_handle = if let Some(secs) = timeout_secs {
        let token = timeout_token.clone();
        Some(tokio::spawn(async move {
            // Use tokio::timeout wrapper around sleep, matching moonrepo exactly
            if timeout(Duration::from_secs(secs), sleep(Duration::from_secs(86400)))
                .await
                .is_err()
            {
                token.cancel();
            }
        }))
    } else {
        None
    };

    // Race between task execution and cancellation (matches moonrepo's tokio::select! pattern)
    let result = tokio::select! {
        // Run conditions in order!
        biased;

        // Cancel if we have timed out
        _ = cancel_clone.cancelled() => {
            Err(miette!("Task '{}' timed out after {} seconds", task_name, timeout_secs.unwrap_or(0)))
        }

        // Or run the task to completion
        r = task_fn(ctx) => r,
    };

    // Cleanup before returning (matches moonrepo pattern)
    if let Some(handle) = timeout_handle {
        handle.abort();
    }

    result
}

impl<'a> Runner<'a> {
    pub fn new(registry: &'a TaskRegistry) -> Self {
        Self {
            registry,
            ran_once: Arc::new(Mutex::new(HashSet::new())),
            task_states: Arc::new(Mutex::new(HashMap::new())),
            operations: Arc::new(Mutex::new(Vec::new())),
            verbose: false,
            jobs: None,
            continue_on_error: false,
        }
    }

    pub fn new_verbose(registry: &'a TaskRegistry) -> Self {
        Self {
            registry,
            ran_once: Arc::new(Mutex::new(HashSet::new())),
            task_states: Arc::new(Mutex::new(HashMap::new())),
            operations: Arc::new(Mutex::new(Vec::new())),
            verbose: true,
            jobs: None,
            continue_on_error: false,
        }
    }

    /// Set the maximum number of concurrent tasks
    pub fn with_jobs(mut self, max_concurrent: usize) -> Self {
        self.jobs = Some(max_concurrent);
        self
    }

    /// Set whether to continue execution on error
    pub fn with_continue_on_error(mut self, continue_on_error: bool) -> Self {
        self.continue_on_error = continue_on_error;
        self
    }

    /// Get all tracked operations
    pub fn get_operations(&self) -> Vec<Operation> {
        self.operations.lock().unwrap().clone()
    }

    /// Get operations for a specific task
    pub fn get_task_operations(&self, task_name: &str) -> Vec<Operation> {
        self.operations
            .lock()
            .unwrap()
            .iter()
            .filter(|op| match &op.op_type {
                OperationType::TaskExecution { task_name: name }
                | OperationType::DependencyCheck {
                    task_name: name, ..
                }
                | OperationType::ConditionCheck { task_name: name } => name == task_name,
                _ => false,
            })
            .cloned()
            .collect()
    }

    /// Log an operation
    fn log_operation(&self, mut operation: Operation) {
        if operation.start_time.is_none() {
            operation.start_time = Some(Instant::now());
        }
        // Finish it if still running (will calculate duration)
        if operation.status == OperationStatus::Running {
            operation.finish(OperationStatus::Passed);
        }
        self.operations.lock().unwrap().push(operation);
    }

    /// Start tracking an operation and return it
    fn start_operation(&self, op_type: OperationType) -> Operation {
        Operation::new(op_type)
    }

    /// Get the current state of a task
    pub fn get_task_state(&self, task_name: &str) -> TaskState {
        self.task_states
            .lock()
            .unwrap()
            .get(task_name)
            .copied()
            .unwrap_or(TaskState::Pending)
    }

    /// Check if all dependencies have completed successfully
    fn are_dependencies_complete(
        task_states: &Arc<Mutex<HashMap<String, TaskState>>>,
        task_name: &str,
        deps: &[String],
        before_hooks: Option<&Vec<String>>,
        wait_for: &[String],
    ) -> miette::Result<bool> {
        let states = task_states.lock().unwrap();

        // Check explicit dependencies
        for dep in deps {
            let dep_state = states.get(dep).copied().unwrap_or(TaskState::Pending);
            if dep_state == TaskState::Failed {
                return Err(miette!(
                    "Task '{}' cannot run: dependency '{}' failed",
                    task_name,
                    dep
                ));
            }
            if !dep_state.is_complete() {
                return Ok(false);
            }
        }

        // Check wait_for tasks (soft dependencies - failures don't block execution)
        for w in wait_for {
            let wait_state = states.get(w).copied().unwrap_or(TaskState::Pending);
            // If wait_for task is still pending, we wait for it
            // But if it failed, we still proceed (soft dependency)
            if !wait_state.is_complete() && wait_state != TaskState::Failed {
                return Ok(false);
            }
        }

        // Check before hooks (they must pass)
        if let Some(hooks) = before_hooks {
            for hook in hooks {
                let hook_state = states.get(hook).copied().unwrap_or(TaskState::Pending);
                if hook_state == TaskState::Failed {
                    return Err(miette!(
                        "Task '{}' cannot run: before hook '{}' failed",
                        task_name,
                        hook
                    ));
                }
                if !hook_state.is_complete() {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Async version of are_dependencies_complete for use with tokio::sync::Mutex
    async fn are_dependencies_complete_async(
        task_states: &Arc<tokio::sync::Mutex<HashMap<String, TaskState>>>,
        task_name: &str,
        deps: &[String],
        before_hooks: Option<&Vec<String>>,
        wait_for: &[String],
    ) -> miette::Result<bool> {
        let states = task_states.lock().await;

        // Check explicit dependencies
        for dep in deps {
            let dep_state = states.get(dep).copied().unwrap_or(TaskState::Pending);
            if dep_state == TaskState::Failed {
                return Err(miette!(
                    "Task '{}' cannot run: dependency '{}' failed",
                    task_name,
                    dep
                ));
            }
            if !dep_state.is_complete() {
                return Ok(false);
            }
        }

        // Check wait_for tasks (soft dependencies - failures don't block execution)
        for w in wait_for {
            let wait_state = states.get(w).copied().unwrap_or(TaskState::Pending);
            // If wait_for task is still pending, we wait for it
            // But if it failed, we still proceed (soft dependency)
            if !wait_state.is_complete() && wait_state != TaskState::Failed {
                return Ok(false);
            }
        }

        // Check before hooks (they must pass)
        if let Some(hooks) = before_hooks {
            for hook in hooks {
                let hook_state = states.get(hook).copied().unwrap_or(TaskState::Pending);
                if hook_state == TaskState::Failed {
                    return Err(miette!(
                        "Task '{}' cannot run: before hook '{}' failed",
                        task_name,
                        hook
                    ));
                }
                if !hook_state.is_complete() {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    pub fn plan(&self, target: &str) -> miette::Result<Vec<String>> {
        let mut op = self.start_operation(OperationType::TaskPlanning {
            target: target.to_string(),
        });

        // Build the dependency graph using the deps module
        let (graph, _node_map) = match build_task_graph(self.registry, target) {
            Ok(result) => result,
            Err(e) => {
                op.finish_with_error(format!("{}", e));
                self.log_operation(op);
                return Err(e);
            }
        };

        // Perform topological sort
        match topological_sort(&graph) {
            Ok(order) => {
                op.finish(OperationStatus::Passed);
                self.log_operation(op);
                Ok(order)
            }
            Err(e) => {
                op.finish_with_error("Cycle detected in task graph");
                self.log_operation(op);
                Err(e)
            }
        }
    }

    /// Group tasks into waves where tasks in the same wave can run in parallel.
    /// Each wave contains tasks whose dependencies (in the previous waves) are all satisfied.
    fn group_into_waves(&self, plan: &[String]) -> Vec<Vec<String>> {
        // Build graph from plan and compute waves using the deps module
        let (graph, _node_map) = build_graph_from_plan(self.registry, plan);
        compute_waves(&graph)
    }

    /// Async version of invoke that uses mise's parallel execution pattern
    /// Adapted from mise's parallelize_tasks for saasctl
    ///
    /// # Arguments
    ///
    /// * `target` - The target task to execute
    /// * `ctx` - The execution context
    /// * `cancellation_token` - Optional cancellation token for graceful shutdown
    /// * `force` - If true, always execute tasks regardless of source changes
    /// * `changed_only` - If true, only execute tasks with changed sources
    pub async fn invoke_async(
        &mut self,
        target: &str,
        ctx: &mut Context,
        cancellation_token: Option<CancellationToken>,
        force: bool,
        changed_only: bool,
    ) -> TaskResult {
        let mut plan = self.plan(target)?;

        // Filter plan for --changed flag
        if changed_only && !force {
            let working_dir = ctx
                .working_path()
                .cloned()
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            let tracker = SourceTracker::new(working_dir);
            plan.retain(|task_name| {
                if let Some(task) = self.registry.get(task_name) {
                    if !task.sources.is_empty() {
                        // Check if sources have changed
                        tracker
                            .has_changed_sources(task_name, &task.sources)
                            .unwrap_or(true) // If error, include task to be safe
                    } else {
                        // No sources defined, include task
                        true
                    }
                } else {
                    // Task not found, include to surface error
                    true
                }
            });
        }

        // Build dependency graph from plan
        let (graph, _node_map) = build_graph_from_plan(self.registry, &plan);
        let main_deps = Arc::new(tokio::sync::Mutex::new(Deps::new(graph)));
        let _num_tasks = main_deps.lock().await.all().count();

        // Determine concurrency limit (default: num_cpus max 8, like mise)
        let jobs = self.jobs.unwrap_or_else(|| {
            let cpus = num_cpus::get();
            cpus.max(8)
        });

        // Convert std mutexes to tokio mutexes for async access
        let ran_once = Arc::new(tokio::sync::Mutex::new(
            self.ran_once.lock().unwrap().clone(),
        ));
        let task_states = Arc::new(tokio::sync::Mutex::new(
            self.task_states.lock().unwrap().clone(),
        ));
        let ctx_arc = Arc::new(tokio::sync::Mutex::new(ctx.clone()));

        // Create source tracker
        let working_dir = ctx
            .working_path()
            .cloned()
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let source_tracker = Arc::new(tokio::sync::Mutex::new(SourceTracker::new(working_dir)));

        // Create TaskExecutor (adapted from mise)
        // Clone registry to avoid lifetime issues in async tasks
        let executor = Arc::new(TaskExecutor::new(
            self.registry.clone(),
            self.continue_on_error,
            self.verbose,
            ran_once.clone(),
            task_states.clone(),
            ctx_arc.clone(),
            cancellation_token,
            source_tracker.clone(),
            force,
            changed_only,
        ));

        // Create scheduler
        let mut scheduler = Scheduler::new(jobs);
        let spawn_context = scheduler.spawn_context();

        // Pump deps leaves into scheduler
        let mut main_done_rx = scheduler.pump_deps(main_deps.clone()).await;

        // Run the scheduler loop (mise's pattern)
        scheduler
            .run_loop(
                &mut main_done_rx,
                main_deps.clone(),
                || executor.is_stopping(),
                self.continue_on_error,
                |task_name, deps_for_remove| {
                    let executor = executor.clone();
                    let spawn_context = spawn_context.clone();
                    async move {
                        TaskExecutor::spawn_sched_job(
                            executor,
                            task_name,
                            deps_for_remove,
                            spawn_context,
                        )
                        .await
                    }
                },
            )
            .await?;

        // Wait for all spawned tasks to complete
        scheduler.join_all(self.continue_on_error).await?;

        // Restore context state and update std mutexes
        let ctx_guard = ctx_arc.lock().await;
        *ctx = ctx_guard.clone();

        // Update std mutexes from tokio mutexes
        *self.ran_once.lock().unwrap() = ran_once.lock().await.clone();
        *self.task_states.lock().unwrap() = task_states.lock().await.clone();

        Ok(())
    }
}
