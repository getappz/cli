//! ScopedFs-backed filesystem host functions for downloadable plugins.
//!
//! Every path is validated through ScopedFs, ensuring plugins cannot escape
//! their project directory sandbox.

use extism::{convert::Json, host_fn};

use crate::wasm::plugin::PluginHostData;
use crate::wasm::types::*;

// ============================================================================
// Read File
// ============================================================================

host_fn!(pub appz_pfs_read_file(
    user_data: PluginHostData;
    args: Json<PluginFsReadInput>
) -> Json<PluginFsReadOutput> {
    let input = args.into_inner();
    let data = user_data.get()?;
    let data = data.lock().unwrap();

    let Some(ref scoped_fs) = data.scoped_fs else {
        return Ok(Json(PluginFsReadOutput {
            success: false,
            content: None,
            error: Some("Filesystem not available for this plugin".to_string()),
        }));
    };

    match scoped_fs.read_to_string(&input.path) {
        Ok(content) => Ok(Json(PluginFsReadOutput {
            success: true,
            content: Some(content),
            error: None,
        })),
        Err(e) => Ok(Json(PluginFsReadOutput {
            success: false,
            content: None,
            error: Some(format!("{}", e)),
        })),
    }
});

// ============================================================================
// Write File
// ============================================================================

host_fn!(pub appz_pfs_write_file(
    user_data: PluginHostData;
    args: Json<PluginFsWriteInput>
) -> Json<PluginFsWriteOutput> {
    let input = args.into_inner();
    let data = user_data.get()?;
    let data = data.lock().unwrap();

    let Some(ref scoped_fs) = data.scoped_fs else {
        return Ok(Json(PluginFsWriteOutput {
            success: false,
            error: Some("Filesystem not available for this plugin".to_string()),
        }));
    };

    match scoped_fs.write_string(&input.path, &input.content) {
        Ok(()) => Ok(Json(PluginFsWriteOutput {
            success: true,
            error: None,
        })),
        Err(e) => Ok(Json(PluginFsWriteOutput {
            success: false,
            error: Some(format!("{}", e)),
        })),
    }
});

// ============================================================================
// Walk Directory (glob)
// ============================================================================

host_fn!(pub appz_pfs_walk_dir(
    user_data: PluginHostData;
    args: Json<PluginFsWalkInput>
) -> Json<PluginFsWalkOutput> {
    let input = args.into_inner();
    let data = user_data.get()?;
    let data = data.lock().unwrap();

    let Some(ref scoped_fs) = data.scoped_fs else {
        return Ok(Json(PluginFsWalkOutput {
            success: false,
            paths: vec![],
            error: Some("Filesystem not available for this plugin".to_string()),
        }));
    };

    let pattern = input.glob.as_deref().unwrap_or("**/*");
    // Combine path + glob
    let full_pattern = if input.path.is_empty() || input.path == "." {
        pattern.to_string()
    } else {
        format!("{}/{}", input.path.trim_end_matches('/'), pattern)
    };

    match scoped_fs.glob(&full_pattern) {
        Ok(entries) => {
            let paths: Vec<String> = entries
                .iter()
                .filter_map(|e| {
                    let p: &std::path::Path = e.as_ref();
                    p.to_str().map(|s| s.to_string())
                })
                .collect();
            Ok(Json(PluginFsWalkOutput {
                success: true,
                paths,
                error: None,
            }))
        }
        Err(e) => Ok(Json(PluginFsWalkOutput {
            success: false,
            paths: vec![],
            error: Some(format!("{}", e)),
        })),
    }
});

// ============================================================================
// Exists
// ============================================================================

host_fn!(pub appz_pfs_exists(
    user_data: PluginHostData;
    args: Json<PluginFsReadInput>
) -> Json<PluginFsExistsOutput> {
    let input = args.into_inner();
    let data = user_data.get()?;
    let data = data.lock().unwrap();

    let Some(ref scoped_fs) = data.scoped_fs else {
        return Ok(Json(PluginFsExistsOutput { exists: false }));
    };

    let exists = scoped_fs.exists(&input.path);
    Ok(Json(PluginFsExistsOutput { exists }))
});

// ============================================================================
// Is File
// ============================================================================

host_fn!(pub appz_pfs_is_file(
    user_data: PluginHostData;
    args: Json<PluginFsReadInput>
) -> Json<PluginFsExistsOutput> {
    let input = args.into_inner();
    let data = user_data.get()?;
    let data = data.lock().unwrap();

    let Some(ref scoped_fs) = data.scoped_fs else {
        return Ok(Json(PluginFsExistsOutput { exists: false }));
    };

    let is_file = scoped_fs.is_file(&input.path);
    Ok(Json(PluginFsExistsOutput { exists: is_file }))
});

// ============================================================================
// Is Dir
// ============================================================================

host_fn!(pub appz_pfs_is_dir(
    user_data: PluginHostData;
    args: Json<PluginFsReadInput>
) -> Json<PluginFsExistsOutput> {
    let input = args.into_inner();
    let data = user_data.get()?;
    let data = data.lock().unwrap();

    let Some(ref scoped_fs) = data.scoped_fs else {
        return Ok(Json(PluginFsExistsOutput { exists: false }));
    };

    let is_dir = scoped_fs.is_dir(&input.path);
    Ok(Json(PluginFsExistsOutput { exists: is_dir }))
});

// ============================================================================
// Mkdir
// ============================================================================

host_fn!(pub appz_pfs_mkdir(
    user_data: PluginHostData;
    args: Json<PluginFsReadInput>
) -> Json<PluginFsWriteOutput> {
    let input = args.into_inner();
    let data = user_data.get()?;
    let data = data.lock().unwrap();

    let Some(ref scoped_fs) = data.scoped_fs else {
        return Ok(Json(PluginFsWriteOutput {
            success: false,
            error: Some("Filesystem not available for this plugin".to_string()),
        }));
    };

    match scoped_fs.create_dir_all(&input.path) {
        Ok(()) => Ok(Json(PluginFsWriteOutput {
            success: true,
            error: None,
        })),
        Err(e) => Ok(Json(PluginFsWriteOutput {
            success: false,
            error: Some(format!("{}", e)),
        })),
    }
});

// ============================================================================
// Copy
// ============================================================================

host_fn!(pub appz_pfs_copy(
    user_data: PluginHostData;
    args: Json<PluginFsCopyInput>
) -> Json<PluginFsWriteOutput> {
    let input = args.into_inner();
    let data = user_data.get()?;
    let data = data.lock().unwrap();

    let Some(ref scoped_fs) = data.scoped_fs else {
        return Ok(Json(PluginFsWriteOutput {
            success: false,
            error: Some("Filesystem not available for this plugin".to_string()),
        }));
    };

    match scoped_fs.copy(&input.source, &input.destination) {
        Ok(()) => Ok(Json(PluginFsWriteOutput {
            success: true,
            error: None,
        })),
        Err(e) => Ok(Json(PluginFsWriteOutput {
            success: false,
            error: Some(format!("{}", e)),
        })),
    }
});

// ============================================================================
// Remove File
// ============================================================================

host_fn!(pub appz_pfs_remove(
    user_data: PluginHostData;
    args: Json<PluginFsReadInput>
) -> Json<PluginFsWriteOutput> {
    let input = args.into_inner();
    let data = user_data.get()?;
    let data = data.lock().unwrap();

    let Some(ref scoped_fs) = data.scoped_fs else {
        return Ok(Json(PluginFsWriteOutput {
            success: false,
            error: Some("Filesystem not available for this plugin".to_string()),
        }));
    };

    match scoped_fs.remove_file(&input.path) {
        Ok(()) => Ok(Json(PluginFsWriteOutput {
            success: true,
            error: None,
        })),
        Err(e) => Ok(Json(PluginFsWriteOutput {
            success: false,
            error: Some(format!("{}", e)),
        })),
    }
});

// ============================================================================
// Remove Dir
// ============================================================================

host_fn!(pub appz_pfs_remove_dir(
    user_data: PluginHostData;
    args: Json<PluginFsReadInput>
) -> Json<PluginFsWriteOutput> {
    let input = args.into_inner();
    let data = user_data.get()?;
    let data = data.lock().unwrap();

    let Some(ref scoped_fs) = data.scoped_fs else {
        return Ok(Json(PluginFsWriteOutput {
            success: false,
            error: Some("Filesystem not available for this plugin".to_string()),
        }));
    };

    match scoped_fs.remove_dir_all(&input.path) {
        Ok(()) => Ok(Json(PluginFsWriteOutput {
            success: true,
            error: None,
        })),
        Err(e) => Ok(Json(PluginFsWriteOutput {
            success: false,
            error: Some(format!("{}", e)),
        })),
    }
});

// ============================================================================
// List Dir
// ============================================================================

host_fn!(pub appz_pfs_list_dir(
    user_data: PluginHostData;
    args: Json<PluginFsReadInput>
) -> Json<PluginFsListDirOutput> {
    let input = args.into_inner();
    let data = user_data.get()?;
    let data = data.lock().unwrap();

    let Some(ref scoped_fs) = data.scoped_fs else {
        return Ok(Json(PluginFsListDirOutput {
            success: false,
            entries: vec![],
            error: Some("Filesystem not available for this plugin".to_string()),
        }));
    };

    match scoped_fs.list_dir(&input.path) {
        Ok(entries) => {
            let result: Vec<PluginFsDirEntry> = entries
                .iter()
                .map(|e| PluginFsDirEntry {
                    name: e.name.clone(),
                    is_file: e.is_file,
                    is_dir: e.is_dir,
                })
                .collect();
            Ok(Json(PluginFsListDirOutput {
                success: true,
                entries: result,
                error: None,
            }))
        }
        Err(e) => Ok(Json(PluginFsListDirOutput {
            success: false,
            entries: vec![],
            error: Some(format!("{}", e)),
        })),
    }
});

// ============================================================================
// Read JSON
// ============================================================================

host_fn!(pub appz_pfs_read_json(
    user_data: PluginHostData;
    args: Json<PluginFsReadInput>
) -> Json<PluginFsJsonOutput> {
    let input = args.into_inner();
    let data = user_data.get()?;
    let data = data.lock().unwrap();

    let Some(ref scoped_fs) = data.scoped_fs else {
        return Ok(Json(PluginFsJsonOutput {
            success: false,
            content: None,
            error: Some("Filesystem not available for this plugin".to_string()),
        }));
    };

    match scoped_fs.read_to_string(&input.path) {
        Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(value) => Ok(Json(PluginFsJsonOutput {
                success: true,
                content: Some(value),
                error: None,
            })),
            Err(e) => Ok(Json(PluginFsJsonOutput {
                success: false,
                content: None,
                error: Some(format!("JSON parse error: {}", e)),
            })),
        },
        Err(e) => Ok(Json(PluginFsJsonOutput {
            success: false,
            content: None,
            error: Some(format!("{}", e)),
        })),
    }
});

// ============================================================================
// Write JSON
// ============================================================================

host_fn!(pub appz_pfs_write_json(
    user_data: PluginHostData;
    args: Json<PluginFsJsonInput>
) -> Json<PluginFsWriteOutput> {
    let input = args.into_inner();
    let data = user_data.get()?;
    let data = data.lock().unwrap();

    let Some(ref scoped_fs) = data.scoped_fs else {
        return Ok(Json(PluginFsWriteOutput {
            success: false,
            error: Some("Filesystem not available for this plugin".to_string()),
        }));
    };

    let Some(ref content) = input.content else {
        return Ok(Json(PluginFsWriteOutput {
            success: false,
            error: Some("No JSON content provided".to_string()),
        }));
    };

    match serde_json::to_string_pretty(content) {
        Ok(json_str) => match scoped_fs.write_string(&input.path, &json_str) {
            Ok(()) => Ok(Json(PluginFsWriteOutput {
                success: true,
                error: None,
            })),
            Err(e) => Ok(Json(PluginFsWriteOutput {
                success: false,
                error: Some(format!("{}", e)),
            })),
        },
        Err(e) => Ok(Json(PluginFsWriteOutput {
            success: false,
            error: Some(format!("JSON serialize error: {}", e)),
        })),
    }
});
