// This file contains code adapted from mise (https://github.com/jdx/mise)
// Original source: src/task/task_scheduler.rs
// License: MIT (Copyright (c) 2025 Jeff Dickey)
// See: C:\Users\shiva\code-ref\mise\LICENSE

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, watch, Mutex, Semaphore};
use tokio::task::JoinSet;

use crate::deps::Deps;
use crate::error::TaskResult;

/// Message type for scheduler: (task_name, deps_graph)
pub type SchedMsg = (String, Arc<Mutex<Deps>>);

/// Schedules and executes tasks with concurrency control using mise's streaming model.
/// Tasks are scheduled as soon as their dependencies complete, rather than in waves.
pub struct Scheduler {
    pub semaphore: Arc<Semaphore>,
    pub jset: Arc<Mutex<JoinSet<TaskResult>>>,
    pub sched_tx: Arc<mpsc::UnboundedSender<SchedMsg>>,
    pub sched_rx: Option<mpsc::UnboundedReceiver<SchedMsg>>,
    pub in_flight: Arc<AtomicUsize>,
}

impl Scheduler {
    /// Create a new scheduler with the specified concurrency limit
    pub fn new(jobs: usize) -> Self {
        let (sched_tx, sched_rx) = mpsc::unbounded_channel::<SchedMsg>();
        Self {
            semaphore: Arc::new(Semaphore::new(jobs)),
            jset: Arc::new(Mutex::new(JoinSet::new())),
            sched_tx: Arc::new(sched_tx),
            sched_rx: Some(sched_rx),
            in_flight: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Take ownership of the receiver (can only be called once)
    pub fn take_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<SchedMsg>> {
        self.sched_rx.take()
    }

    /// Wait for all spawned tasks to complete
    /// Note: in_flight is now decremented when tasks complete, not here
    /// This function just ensures all tasks are joined and handles errors
    pub async fn join_all(&self, continue_on_error: bool) -> TaskResult {
        let mut jset_guard = self.jset.lock().await;
        while let Some(result) = jset_guard.join_next().await {
            match result {
                Ok(Ok(())) => {
                    // Task succeeded - in_flight already decremented in task
                }
                Ok(Err(e)) => {
                    // Task failed - in_flight already decremented in task
                    if !continue_on_error {
                        // Cancel remaining tasks
                        jset_guard.abort_all();
                        return Err(e);
                    }
                }
                Err(e) => {
                    // Join error (task panicked or was cancelled)
                    // Decrement in_flight here since task didn't complete normally
                    self.in_flight.fetch_sub(1, Ordering::SeqCst);
                    if !continue_on_error {
                        jset_guard.abort_all();
                        return Err(miette::miette!("Task join failed: {}", e));
                    }
                }
            }
        }
        Ok(())
    }

    /// Create a spawn context for passing to spawned tasks
    pub fn spawn_context(&self) -> SpawnContext {
        SpawnContext {
            semaphore: self.semaphore.clone(),
            sched_tx: self.sched_tx.clone(),
            jset: self.jset.clone(),
            in_flight: self.in_flight.clone(),
        }
    }

    /// Get the current number of in-flight tasks
    pub fn in_flight_count(&self) -> usize {
        self.in_flight.load(Ordering::SeqCst)
    }

    /// Pump dependency graph leaves into the scheduler.
    ///
    /// Forwards initial leaves synchronously, then spawns an async task to forward
    /// remaining leaves as they become available. Returns a watch receiver that signals
    /// when all dependencies are complete.
    pub async fn pump_deps(&self, deps: Arc<Mutex<Deps>>) -> watch::Receiver<bool> {
        let (main_done_tx, main_done_rx) = watch::channel(false);
        let sched_tx = self.sched_tx.clone();
        let deps_clone = deps.clone();

        // Get subscription and forward initial leaves synchronously
        let mut rx = {
            let mut deps_guard = deps_clone.lock().await;
            deps_guard.subscribe()
        };

        // Check if graph is already empty (no tasks)
        let is_empty = {
            let deps_guard = deps_clone.lock().await;
            deps_guard.is_empty()
        };

        if is_empty {
            // No tasks to run, signal completion immediately
            let _ = main_done_tx.send(true);
        } else {
            // Drain initial messages
            let mut saw_none = false;
            loop {
                match rx.try_recv() {
                    Ok(Some(task_name)) => {
                        // trace!("main deps initial leaf: {}", task_name);
                        let _ = sched_tx.send((task_name, deps_clone.clone()));
                    }
                    Ok(None) => {
                        // trace!("main deps initial done");
                        saw_none = true;
                        break;
                    }
                    Err(mpsc::error::TryRecvError::Empty) => {
                        break;
                    }
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        break;
                    }
                }
            }

            // If we saw None during initial drain, signal completion immediately
            if saw_none {
                let _ = main_done_tx.send(true);
            } else {
                // Forward remaining leaves asynchronously using the same subscription
                tokio::spawn(async move {
                    while let Some(msg) = rx.recv().await {
                        match msg {
                            Some(task_name) => {
                                // trace!("main deps leaf scheduled: {}", task_name);
                                let _ = sched_tx.send((task_name, deps_clone.clone()));
                            }
                            None => {
                                // trace!("main deps completed");
                                let _ = main_done_tx.send(true);
                                break;
                            }
                        }
                    }
                });
            }
        }

        main_done_rx
    }

    /// Run the scheduler loop, draining tasks and spawning them via the callback.
    ///
    /// The loop continues until:
    /// - main_done signal is received AND
    /// - no tasks are in-flight AND
    /// - no tasks were recently drained
    ///
    /// Or if should_stop returns true (for early exit due to failures)
    pub async fn run_loop<F, Fut>(
        &mut self,
        main_done_rx: &mut watch::Receiver<bool>,
        main_deps: Arc<Mutex<Deps>>,
        should_stop: impl Fn() -> bool,
        continue_on_error: bool,
        mut spawn_job: F,
    ) -> TaskResult
    where
        F: FnMut(String, Arc<Mutex<Deps>>) -> Fut,
        Fut: std::future::Future<Output = TaskResult>,
    {
        let mut sched_rx = self.take_receiver().expect("receiver already taken");

        loop {
            // Drain ready tasks without awaiting
            let mut drained_any = false;
            loop {
                match sched_rx.try_recv() {
                    Ok((task_name, deps_for_remove)) => {
                        drained_any = true;
                        // trace!("scheduler received: {}", task_name);
                        if should_stop() && !continue_on_error {
                            break;
                        }
                        spawn_job(task_name, deps_for_remove).await?;
                    }
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => break,
                }
            }

            // Check if we should stop early due to failure
            if should_stop() && !continue_on_error {
                // trace!("scheduler: stopping early due to failure, cleaning up main deps");
                // Clean up the dependency graph to ensure the main_done signal is sent
                let mut deps = main_deps.lock().await;
                let tasks_to_remove: Vec<String> = deps.all().cloned().collect();
                for task_name in tasks_to_remove {
                    deps.remove(&task_name);
                }
                drop(deps);
                break;
            }

            // Try to join any completed tasks from the JoinSet
            // This ensures we don't wait forever if tasks have completed but haven't been joined
            {
                let mut jset_guard = self.jset.lock().await;
                while let Some(result) = jset_guard.try_join_next() {
                    match result {
                        Ok(Ok(())) => {
                            // Task succeeded - in_flight already decremented in task
                        }
                        Ok(Err(e)) => {
                            // Task failed - in_flight already decremented in task
                            if !continue_on_error {
                                jset_guard.abort_all();
                                return Err(e);
                            }
                        }
                        Err(e) => {
                            // Join error (task panicked or was cancelled)
                            self.in_flight.fetch_sub(1, Ordering::SeqCst);
                            if !continue_on_error {
                                jset_guard.abort_all();
                                return Err(miette::miette!("Task join failed: {}", e));
                            }
                        }
                    }
                }
            }

            // Exit if main deps finished and nothing is running/queued
            // Check both in_flight count AND if JoinSet is empty (tasks may have completed)
            let jset_empty = {
                let jset_guard = self.jset.lock().await;
                jset_guard.is_empty()
            };

            let main_done = *main_done_rx.borrow();
            let in_flight = self.in_flight_count();

            if main_done && in_flight == 0 && jset_empty && !drained_any {
                // trace!("scheduler drain complete; exiting loop");
                break;
            }

            // Await either new work or main_done change, but also poll for task completions
            // Use a timeout to periodically check for completed tasks
            tokio::select! {
                m = sched_rx.recv() => {
                    if let Some((task_name, deps_for_remove)) = m {
                        // trace!("scheduler received: {}", task_name);
                        if should_stop() && !continue_on_error {
                            break;
                        }
                        spawn_job(task_name, deps_for_remove).await?;
                    } else {
                        // channel closed; rely on main_done/in_flight to exit soon
                        // Check if we should exit now
                        if *main_done_rx.borrow() && self.in_flight_count() == 0 {
                            let jset_guard = self.jset.lock().await;
                            if jset_guard.is_empty() {
                                break;
                            }
                        }
                    }
                }
                _ = main_done_rx.changed() => {
                    // trace!("main_done changed: {}", *main_done_rx.borrow());
                    // When main_done changes, check if we can exit
                    if *main_done_rx.borrow() && self.in_flight_count() == 0 {
                        let jset_guard = self.jset.lock().await;
                        if jset_guard.is_empty() && !drained_any {
                            break;
                        }
                    }
                }
                // Poll for task completions with a short timeout
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)) => {
                    // Try to join any completed tasks
                    let mut jset_guard = self.jset.lock().await;
                    while let Some(result) = jset_guard.try_join_next() {
                        match result {
                            Ok(Ok(())) => {
                                // Task succeeded
                            }
                            Ok(Err(e)) => {
                                // Task failed
                                if !continue_on_error {
                                    jset_guard.abort_all();
                                    drop(jset_guard);
                                    return Err(e);
                                }
                            }
                            Err(e) => {
                                // Join error
                                self.in_flight.fetch_sub(1, Ordering::SeqCst);
                                if !continue_on_error {
                                    jset_guard.abort_all();
                                    drop(jset_guard);
                                    return Err(miette::miette!("Task join failed: {}", e));
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// Context passed to spawned tasks
#[derive(Clone)]
pub struct SpawnContext {
    pub semaphore: Arc<Semaphore>,
    pub sched_tx: Arc<mpsc::UnboundedSender<SchedMsg>>,
    pub jset: Arc<Mutex<JoinSet<TaskResult>>>,
    pub in_flight: Arc<AtomicUsize>,
}
