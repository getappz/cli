use futures::future::BoxFuture;
use std::sync::Arc;

use crate::{context::Context, error::TaskResult};

/// Primary async task function type
/// Matches moonrepo's pattern: tasks receive Arc<Context> and return BoxFuture
pub type AsyncTaskFn = Arc<dyn Fn(Arc<Context>) -> BoxFuture<'static, TaskResult> + Send + Sync>;

/// Macro for defining async tasks
/// Usage: task_fn_async!(|ctx: Arc<Context>| async move { ... })
#[macro_export]
macro_rules! task_fn_async {
    ($f:expr) => {
        std::sync::Arc::new(move |ctx: std::sync::Arc<$crate::context::Context>| {
            Box::pin(async move { $f(ctx).await })
                as futures::future::BoxFuture<'static, $crate::error::TaskResult>
        }) as $crate::types::AsyncTaskFn
    };
}

/// Macro for wrapping synchronous tasks
/// Usage: task_fn_sync!(|ctx: Arc<Context>| { ... })
/// Internally wraps with tokio::task::spawn_blocking
#[macro_export]
macro_rules! task_fn_sync {
    ($f:expr) => {
        std::sync::Arc::new(move |ctx: std::sync::Arc<$crate::context::Context>| {
            Box::pin(async move { ($f)(ctx) })
                as futures::future::BoxFuture<'static, $crate::error::TaskResult>
        }) as $crate::types::AsyncTaskFn
    };
}
