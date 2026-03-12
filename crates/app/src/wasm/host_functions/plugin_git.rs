//! Git host functions for downloadable plugins.
//!
//! Git operations are executed through the sandbox provider or by checking
//! the ScopedFs for `.git` directory existence.

use extism::{convert::Json, host_fn};

use crate::wasm::plugin::PluginHostData;
use crate::wasm::types::*;

// ============================================================================
// Git Changed Files
// ============================================================================

host_fn!(pub appz_pgit_changed_files(
    user_data: PluginHostData;
    args: Json<PluginFsReadInput>
) -> Json<PluginGitFilesOutput> {
    let _input = args.into_inner();
    let data = user_data.get()?;
    let data = data.lock().unwrap();

    let Some(ref sandbox) = data.sandbox else {
        return Ok(Json(PluginGitFilesOutput {
            success: false,
            files: vec![],
            error: Some("Sandbox not available for this plugin".to_string()),
        }));
    };

    // Use sandbox exec to run git diff
    let sandbox = sandbox.clone();
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            sandbox.exec("git diff --name-only").await
        })
    });

    match result {
        Ok(output) => {
            let stdout = output.stdout();
            let files: Vec<String> = stdout
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.to_string())
                .collect();
            Ok(Json(PluginGitFilesOutput {
                success: true,
                files,
                error: None,
            }))
        }
        Err(e) => Ok(Json(PluginGitFilesOutput {
            success: false,
            files: vec![],
            error: Some(format!("{}", e)),
        })),
    }
});

// ============================================================================
// Git Staged Files
// ============================================================================

host_fn!(pub appz_pgit_staged_files(
    user_data: PluginHostData;
    args: Json<PluginFsReadInput>
) -> Json<PluginGitFilesOutput> {
    let _input = args.into_inner();
    let data = user_data.get()?;
    let data = data.lock().unwrap();

    let Some(ref sandbox) = data.sandbox else {
        return Ok(Json(PluginGitFilesOutput {
            success: false,
            files: vec![],
            error: Some("Sandbox not available for this plugin".to_string()),
        }));
    };

    let sandbox = sandbox.clone();
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            sandbox.exec("git diff --staged --name-only").await
        })
    });

    match result {
        Ok(output) => {
            let stdout = output.stdout();
            let files: Vec<String> = stdout
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.to_string())
                .collect();
            Ok(Json(PluginGitFilesOutput {
                success: true,
                files,
                error: None,
            }))
        }
        Err(e) => Ok(Json(PluginGitFilesOutput {
            success: false,
            files: vec![],
            error: Some(format!("{}", e)),
        })),
    }
});

// ============================================================================
// Git Is Repo
// ============================================================================

host_fn!(pub appz_pgit_is_repo(
    user_data: PluginHostData;
    _args: Json<PluginFsReadInput>
) -> Json<PluginGitIsRepoOutput> {
    let data = user_data.get()?;
    let data = data.lock().unwrap();

    let Some(ref scoped_fs) = data.scoped_fs else {
        return Ok(Json(PluginGitIsRepoOutput { is_repo: false }));
    };

    let is_repo = scoped_fs.is_dir(".git");
    Ok(Json(PluginGitIsRepoOutput { is_repo }))
});
