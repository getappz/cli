//! Helper macros for reducing plugin boilerplate

/// Helper macro to simplify task registration
///
/// # Example
///
/// ```rust,ignore
/// appz_task!("deploy", "Deploy the application");
/// appz_task!("deploy", "Deploy the application", deps: vec!["setup".to_string()]);
/// ```
#[macro_export]
macro_rules! appz_task {
    ($name:expr) => {
        appz_task!($name, "", None);
    };
    ($name:expr, $desc:expr) => {{
        use $crate::TaskInput;
        unsafe {
            appz_reg_task(Json(TaskInput {
                name: $name.to_string(),
                desc: Some($desc.to_string()),
                deps: None,
                body: None,
                only_if: None,
                unless: None,
                once: None,
                hidden: None,
                timeout: None,
            }))
        }
    }};
    ($name:expr, $desc:expr, deps: $deps:expr) => {{
        use $crate::TaskInput;
        unsafe {
            appz_reg_task(Json(TaskInput {
                name: $name.to_string(),
                desc: Some($desc.to_string()),
                deps: Some($deps),
                body: None,
                only_if: None,
                unless: None,
                once: None,
                hidden: None,
                timeout: None,
            }))
        }
    }};
}

/// Helper macro to set context values
///
/// # Example
///
/// ```rust,ignore
/// appz_set!("key", "value");
/// ```
#[macro_export]
macro_rules! appz_set {
    ($key:expr, $value:expr) => {{
        use $crate::ContextSetInput;
        unsafe {
            appz_ctx_set(Json(ContextSetInput {
                key: $key.to_string(),
                value: $value.to_string(),
            }))
        }
    }};
}

/// Helper macro to get context values
///
/// # Example
///
/// ```rust,ignore
/// let value = appz_get!("key");
/// ```
#[macro_export]
macro_rules! appz_get {
    ($key:expr) => {{
        unsafe {
            match appz_ctx_get($key.to_string()) {
                Ok(Json(output)) => output.value,
                Err(_) => None,
            }
        }
    }};
}

/// Helper macro to run commands
///
/// # Example
///
/// ```rust,ignore
/// appz_run!("echo hello");
/// ```
#[macro_export]
macro_rules! appz_run {
    ($cmd:expr) => {{
        use $crate::RunInput;
        unsafe {
            appz_exec_run_local(Json(RunInput {
                command: $cmd.to_string(),
                cwd: None,
                env: None,
                secret: None,
                nothrow: None,
                force_output: None,
                timeout: None,
                idle_timeout: None,
            }))
        }
    }};
}

/// Helper macro to invoke tasks
///
/// # Example
///
/// ```rust,ignore
/// appz_invoke!("setup");
/// ```
#[macro_export]
macro_rules! appz_invoke {
    ($task:expr) => {{
        use $crate::InvokeInput;
        unsafe {
            appz_exec_invoke(Json(InvokeInput {
                task: $task.to_string(),
            }))
        }
    }};
}

/// Helper macro for logging
#[macro_export]
macro_rules! appz_info {
    ($($arg:tt)*) => {
        {
            unsafe {
                let _ = appz_util_info(format!($($arg)*));
            }
        }
    };
}

/// Helper macro for warnings
#[macro_export]
macro_rules! appz_warning {
    ($($arg:tt)*) => {
        {
            unsafe {
                let _ = appz_util_warning(format!($($arg)*));
            }
        }
    };
}
