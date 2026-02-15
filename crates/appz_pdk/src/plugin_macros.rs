//! Helper macros for downloadable plugin host function calls.
//!
//! These macros simplify calling the sandbox-backed host functions
//! from within WASM plugin code.

/// Read a file through the sandboxed filesystem.
///
/// # Example
/// ```rust,ignore
/// let content = appz_fs_read!("src/App.tsx")?;
/// ```
#[macro_export]
macro_rules! appz_fs_read {
    ($path:expr) => {{
        use $crate::PluginFsReadInput;
        unsafe {
            appz_pfs_read_file(Json(PluginFsReadInput {
                path: $path.to_string(),
            }))
        }
    }};
}

/// Write a file through the sandboxed filesystem.
///
/// # Example
/// ```rust,ignore
/// appz_fs_write!("output/index.astro", content)?;
/// ```
#[macro_export]
macro_rules! appz_fs_write {
    ($path:expr, $content:expr) => {{
        use $crate::PluginFsWriteInput;
        unsafe {
            appz_pfs_write_file(Json(PluginFsWriteInput {
                path: $path.to_string(),
                content: $content.to_string(),
            }))
        }
    }};
}

/// Walk a directory with an optional glob pattern.
///
/// # Example
/// ```rust,ignore
/// let files = appz_fs_walk!("src", "**/*.tsx")?;
/// ```
#[macro_export]
macro_rules! appz_fs_walk {
    ($path:expr) => {{
        use $crate::PluginFsWalkInput;
        unsafe {
            appz_pfs_walk_dir(Json(PluginFsWalkInput {
                path: $path.to_string(),
                glob: None,
            }))
        }
    }};
    ($path:expr, $glob:expr) => {{
        use $crate::PluginFsWalkInput;
        unsafe {
            appz_pfs_walk_dir(Json(PluginFsWalkInput {
                path: $path.to_string(),
                glob: Some($glob.to_string()),
            }))
        }
    }};
}

/// Check if a path exists in the sandboxed filesystem.
///
/// # Example
/// ```rust,ignore
/// let exists = appz_fs_exists!("package.json")?;
/// ```
#[macro_export]
macro_rules! appz_fs_exists {
    ($path:expr) => {{
        use $crate::PluginFsReadInput;
        unsafe {
            appz_pfs_exists(Json(PluginFsReadInput {
                path: $path.to_string(),
            }))
        }
    }};
}

/// Get git changed files through the sandbox.
///
/// # Example
/// ```rust,ignore
/// let files = appz_git_changed!()?;
/// ```
#[macro_export]
macro_rules! appz_git_changed {
    () => {{
        use $crate::PluginFsReadInput;
        unsafe {
            appz_pgit_changed_files(Json(PluginFsReadInput {
                path: ".".to_string(),
            }))
        }
    }};
}

/// Execute a command through the sandbox.
///
/// # Example
/// ```rust,ignore
/// let output = appz_sandbox_exec!("bun install")?;
/// ```
#[macro_export]
macro_rules! appz_sandbox_exec {
    ($cmd:expr) => {{
        use $crate::PluginSandboxExecInput;
        unsafe {
            appz_psandbox_exec(Json(PluginSandboxExecInput {
                command: $cmd.to_string(),
            }))
        }
    }};
}

/// Apply AST transformation rules to code.
///
/// # Example
/// ```rust,ignore
/// let result = appz_ast_transform!(code, rules)?;
/// ```
#[macro_export]
macro_rules! appz_ast_transform {
    ($code:expr, $rules:expr) => {{
        use $crate::PluginAstTransformInput;
        unsafe {
            appz_past_transform(Json(PluginAstTransformInput {
                code: $code.to_string(),
                rules: $rules,
            }))
        }
    }};
}
