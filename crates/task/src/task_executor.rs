// This file contains code adapted from mise (https://github.com/jdx/mise)
// Original source: src/task/task_executor.rs
// License: MIT (Copyright (c) 2025 Jeff Dickey)
// See: C:\Users\shiva\code-ref\mise\LICENSE

use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex as StdMutex};

use miette::miette;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use tokio_util::sync::CancellationToken;

use crate::{
    context::Context, deps::Deps, error::TaskResult, registry::TaskRegistry, runner::TaskState,
    scheduler::SpawnContext, source_tracker::SourceTracker, types::AsyncTaskFn,
};

/// Type for tracking failed tasks
pub type FailedTasks = Arc<StdMutex<Vec<(String, Option<i32>)>>>;

/// Executes tasks with proper context and error handling
/// Adapted from mise's TaskExecutor for saasctl's AsyncTaskFn model
pub struct TaskExecutor {
    pub registry: TaskRegistry,
    pub failed_tasks: FailedTasks,
    pub continue_on_error: bool,
    pub verbose: bool,
    pub ran_once: Arc<Mutex<HashSet<String>>>,
    pub task_states: Arc<Mutex<std::collections::HashMap<String, TaskState>>>,
    pub ctx: Arc<Mutex<Context>>,
    pub cancellation_token: Option<CancellationToken>,
    pub source_tracker: Arc<tokio::sync::Mutex<SourceTracker>>,
    pub force: bool,
    pub changed_only: bool,
}

impl TaskExecutor {
    pub fn new(
        registry: TaskRegistry,
        continue_on_error: bool,
        verbose: bool,
        ran_once: Arc<Mutex<HashSet<String>>>,
        task_states: Arc<Mutex<std::collections::HashMap<String, TaskState>>>,
        ctx: Arc<Mutex<Context>>,
        cancellation_token: Option<CancellationToken>,
        source_tracker: Arc<tokio::sync::Mutex<SourceTracker>>,
        force: bool,
        changed_only: bool,
    ) -> Self {
        Self {
            registry,
            failed_tasks: Arc::new(StdMutex::new(Vec::new())),
            continue_on_error,
            verbose,
            ran_once,
            task_states,
            ctx,
            cancellation_token,
            source_tracker,
            force,
            changed_only,
        }
    }

    pub fn is_stopping(&self) -> bool {
        !self.failed_tasks.lock().unwrap().is_empty()
    }

    pub fn add_failed_task(&self, task_name: String, status: Option<i32>) {
        let mut failed = self.failed_tasks.lock().unwrap();
        failed.push((task_name, status.or(Some(1))));
    }

    /// Run a task by calling its AsyncTaskFn
    /// This replaces mise's script execution with our function execution
    async fn run_task_sched(&self, task_name: &str, ctx: Arc<Context>) -> TaskResult {
        let task = self
            .registry
            .get(task_name)
            .ok_or_else(|| miette!("Task '{}' not found", task_name))?;

        // Execute the task's AsyncTaskFn
        let result = (task.action)(ctx.clone()).await;

        match &result {
            Ok(_) => {
                if self.verbose {
                    println!("✓ {}", task_name);
                }
            }
            Err(_) => {
                if self.verbose {
                    eprintln!("✗ {}", task_name);
                }
                self.add_failed_task(task_name.to_string(), None);
            }
        }

        result
    }

    /// Execute task with timeout support
    async fn execute_task_with_timeout(
        task_name: &str,
        task_timeout: Option<u64>,
        task_action: AsyncTaskFn,
        ctx: Arc<Context>,
    ) -> TaskResult {
        let future = (task_action)(ctx);

        if let Some(timeout_secs) = task_timeout {
            timeout(Duration::from_secs(timeout_secs), future)
                .await
                .map_err(|_| {
                    miette!(
                        "Task '{}' timed out after {} seconds",
                        task_name,
                        timeout_secs
                    )
                })?
        } else {
            future.await
        }
    }

    /// Check if dependencies are complete
    async fn are_dependencies_complete(
        task_states: &Arc<Mutex<std::collections::HashMap<String, TaskState>>>,
        task_name: &str,
        deps: &[String],
        before_hooks: Option<&Vec<String>>,
        wait_for: &[String],
    ) -> Result<bool, miette::Error> {
        let states = task_states.lock().await;

        // Check hard dependencies
        for dep in deps {
            match states.get(dep) {
                Some(TaskState::Passed) | Some(TaskState::Skipped) => continue,
                Some(TaskState::Failed) => {
                    return Err(miette!("Dependency '{}' failed", dep));
                }
                _ => {
                    // Dependency not complete yet - this is expected in streaming model
                    // The task should only be scheduled when dependencies are ready
                    // But we check here as a safety measure
                    return Ok(false);
                }
            }
        }

        // Check before hooks
        if let Some(hooks) = before_hooks {
            for hook in hooks {
                match states.get(hook) {
                    Some(TaskState::Passed) | Some(TaskState::Skipped) => continue,
                    Some(TaskState::Failed) => {
                        return Err(miette!("Hook '{}' failed", hook));
                    }
                    _ => {
                        // Hook not complete yet - return false so task waits
                        return Ok(false);
                    }
                }
            }
        }

        // Check wait_for (soft dependencies)
        // From mise: wait_for failures don't block execution (truly soft)
        // Only check if wait_for task is still pending - if it failed, we proceed anyway
        for wait in wait_for {
            match states.get(wait) {
                Some(TaskState::Passed) | Some(TaskState::Skipped) => {
                    // Wait-for task completed successfully - we can proceed
                    continue;
                }
                Some(TaskState::Failed) => {
                    // Wait-for task failed - but this is soft, so we proceed anyway
                    // Don't block execution for wait_for failures
                    continue;
                }
                _ => {
                    // Wait-for task is still pending - wait for it
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Spawn a scheduled job (adapted from mise's spawn_sched_job)
    /// This is the critical pattern that ensures proper in_flight tracking
    pub async fn spawn_sched_job(
        this: Arc<Self>,
        task_name: String,
        deps_for_remove: Arc<Mutex<Deps>>,
        ctx: SpawnContext,
    ) -> TaskResult {
        // If we're already stopping due to a previous failure and not in
        // continue-on-error mode, do not launch this task. Ensure we remove
        // it from the dependency graph so the scheduler can make progress.
        if this.is_stopping() && !this.continue_on_error {
            deps_for_remove.lock().await.remove(&task_name);
            return Ok(());
        }

        // Get task info before spawning
        let task_info = this
            .registry
            .get(&task_name)
            .ok_or_else(|| miette!("Task '{}' not found", task_name))?;

        let task_deps = task_info.deps.clone();
        let task_wait_for = task_info.wait_for.clone();
        let task_action = task_info.action.clone();
        let task_timeout = task_info.timeout;
        let task_once = task_info.once;
        let task_only_if = task_info.only_if.clone();
        let task_unless = task_info.unless.clone();
        let task_sources = task_info.sources.clone();
        let task_outputs = task_info.outputs.clone();
        let before_hooks = this.registry.hooks.before.get(&task_name).cloned();

        // Acquire semaphore permit BEFORE spawning (critical for concurrency control)
        let permit = ctx
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| miette!("Failed to acquire semaphore permit: {}", e))?;

        // If a failure occurred while we were waiting for a permit and we're not
        // in continue-on-error mode, skip launching this task.
        if this.is_stopping() && !this.continue_on_error {
            // Remove from deps so the scheduler can drain and not hang
            deps_for_remove.lock().await.remove(&task_name);
            return Ok(());
        }

        // Increment in_flight BEFORE spawning (critical pattern from mise)
        ctx.in_flight.fetch_add(1, Ordering::SeqCst);
        let in_flight_c = ctx.in_flight.clone();

        let task_name_clone = task_name.clone();
        let this_clone = this.clone();

        // Spawn the task
        ctx.jset.lock().await.spawn(async move {
            let _permit = permit; // Hold permit for task lifetime

            // Check for cancellation
            if let Some(ref token) = this_clone.cancellation_token {
                if token.is_cancelled() {
                    deps_for_remove.lock().await.remove(&task_name_clone);
                    in_flight_c.fetch_sub(1, Ordering::SeqCst);
                    return Err(miette!("Task '{}' cancelled", task_name_clone));
                }
            }

            // Check ran_once flag
            {
                let ran_set = this_clone.ran_once.lock().await;
                if task_once && ran_set.contains(&task_name_clone) {
                    let mut states = this_clone.task_states.lock().await;
                    states.insert(task_name_clone.clone(), TaskState::Skipped);
                    deps_for_remove.lock().await.remove(&task_name_clone);
                    in_flight_c.fetch_sub(1, Ordering::SeqCst);
                    return Ok(());
                }
            }

            // Check should_run condition
            {
                let ctx_guard = this_clone.ctx.lock().await;
                let should_run = task_only_if.iter().all(|c| c(&ctx_guard))
                    && !task_unless.iter().any(|c| c(&ctx_guard));
                if !should_run {
                    let mut states = this_clone.task_states.lock().await;
                    states.insert(task_name_clone.clone(), TaskState::Skipped);
                    deps_for_remove.lock().await.remove(&task_name_clone);
                    in_flight_c.fetch_sub(1, Ordering::SeqCst);
                    return Ok(());
                }
            }

            // Check dependencies
            match TaskExecutor::are_dependencies_complete(
                &this_clone.task_states,
                &task_name_clone,
                &task_deps,
                before_hooks.as_ref().map(|v| v as &Vec<String>),
                &task_wait_for,
            )
            .await
            {
                Ok(true) => {
                    // Dependencies are complete - proceed with execution
                }
                Ok(false) => {
                    // Dependencies not ready yet - this shouldn't happen in streaming model
                    // but handle gracefully
                    let mut states = this_clone.task_states.lock().await;
                    states.insert(task_name_clone.clone(), TaskState::Skipped);
                    deps_for_remove.lock().await.remove(&task_name_clone);
                    in_flight_c.fetch_sub(1, Ordering::SeqCst);
                    return Ok(());
                }
                Err(err) => {
                    let mut states = this_clone.task_states.lock().await;
                    states.insert(task_name_clone.clone(), TaskState::Failed);
                    this_clone.add_failed_task(task_name_clone.clone(), None);
                    deps_for_remove.lock().await.remove(&task_name_clone);
                    in_flight_c.fetch_sub(1, Ordering::SeqCst);
                    return Err(err);
                }
            }

            // Check source/output tracking (skip if sources haven't changed)
            if !this_clone.force && (!task_sources.is_empty() || !task_outputs.is_empty()) {
                let tracker = this_clone.source_tracker.lock().await;
                match tracker.should_skip_task(&task_name_clone, &task_sources, &task_outputs) {
                    Ok(true) => {
                        // Task should be skipped - sources are up-to-date
                        if this_clone.verbose {
                            println!("⊘ {} (skipped - sources unchanged)", task_name_clone);
                        }
                        let mut states = this_clone.task_states.lock().await;
                        states.insert(task_name_clone.clone(), TaskState::Skipped);
                        deps_for_remove.lock().await.remove(&task_name_clone);
                        in_flight_c.fetch_sub(1, Ordering::SeqCst);
                        return Ok(());
                    }
                    Ok(false) => {
                        // Task should run - sources changed or outputs missing
                    }
                    Err(e) => {
                        // Error checking sources/outputs - log and run anyway
                        if this_clone.verbose {
                            eprintln!(
                                "Warning: Failed to check sources/outputs for {}: {}",
                                task_name_clone, e
                            );
                        }
                    }
                }
            }

            // Note: confirm is handled in the task action wrapper (in app crate)
            // This follows mise's pattern but keeps ui dependency in app layer

            // Print start message
            if this_clone.verbose {
                println!("→ {}", task_name_clone);
            }

            // Get context for execution
            let ctx_arc = {
                let ctx_guard = this_clone.ctx.lock().await;
                Arc::new(ctx_guard.clone())
            };

            // Execute the task
            let result = TaskExecutor::execute_task_with_timeout(
                &task_name_clone,
                task_timeout,
                task_action,
                ctx_arc,
            )
            .await;

            // Handle result
            match result {
                Err(err) => {
                    // Store error and mark as failed
                    let mut states = this_clone.task_states.lock().await;
                    states.insert(task_name_clone.clone(), TaskState::Failed);
                    this_clone.add_failed_task(task_name_clone.clone(), None);
                    if this_clone.verbose {
                        eprintln!("✗ {}", task_name_clone);
                    }
                    deps_for_remove.lock().await.remove(&task_name_clone);
                    in_flight_c.fetch_sub(1, Ordering::SeqCst);
                    Err(err)
                }
                Ok(_) => {
                    // Mark as passed and ran_once if needed
                    let mut states = this_clone.task_states.lock().await;
                    states.insert(task_name_clone.clone(), TaskState::Passed);
                    if task_once {
                        this_clone
                            .ran_once
                            .lock()
                            .await
                            .insert(task_name_clone.clone());
                    }

                    // Record task execution in source tracker cache
                    if !task_sources.is_empty() || !task_outputs.is_empty() {
                        let mut tracker = this_clone.source_tracker.lock().await;
                        if let Err(e) = tracker.record_task_execution(
                            &task_name_clone,
                            &task_sources,
                            &task_outputs,
                        ) {
                            if this_clone.verbose {
                                eprintln!(
                                    "Warning: Failed to record task execution for {}: {}",
                                    task_name_clone, e
                                );
                            }
                        }
                    }

                    // Remove from deps graph (triggers new leaves to be emitted)
                    deps_for_remove.lock().await.remove(&task_name_clone);

                    // Decrement in_flight at the VERY END (critical pattern from mise)
                    // This must happen after deps.remove() to ensure proper sequencing
                    in_flight_c.fetch_sub(1, Ordering::SeqCst);

                    Ok(())
                }
            }
        });

        Ok(())
    }
}
