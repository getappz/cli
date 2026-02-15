//! File copying helpers for Astro project generation.

use camino::Utf8PathBuf;
use miette::{miette, Result};
use std::fs;

pub(super) fn copy_tailwind_config(source_dir: &Utf8PathBuf, output_dir: &Utf8PathBuf) -> Result<()> {
    for ext in ["ts", "js"] {
        let src = source_dir.join(format!("tailwind.config.{ext}"));
        if src.exists() {
            let dst = output_dir.join(format!("tailwind.config.{ext}"));
            fs::copy(&src, &dst)
                .map_err(|e| miette!("Failed to copy tailwind.config.{}: {}", ext, e))?;
        }
    }
    Ok(())
}

pub(super) fn copy_public_assets(source_dir: &Utf8PathBuf, public_dir: &Utf8PathBuf) -> Result<()> {
    let source_public = source_dir.join("public");
    if source_public.exists() {
        copy_dir_all(&source_public, public_dir)?;
    }
    Ok(())
}

pub(super) fn copy_css_files(source_src_dir: &Utf8PathBuf, dest_src_dir: &Utf8PathBuf) -> Result<()> {
    for entry in fs::read_dir(source_src_dir).map_err(|e| miette!("Failed to read src: {}", e))? {
        let entry = entry.map_err(|e| miette!("Failed to read entry: {}", e))?;
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |e| e == "css") {
            let file_name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| miette!("Invalid file name"))?;
            let dest_path = dest_src_dir.join(file_name);
            fs::copy(&path, &dest_path)
                .map_err(|e| miette!("Failed to copy CSS {}: {}", file_name, e))?;
        }
    }
    Ok(())
}

pub(super) fn copy_dir_all(src: &Utf8PathBuf, dst: &Utf8PathBuf) -> Result<()> {
    fs::create_dir_all(dst).map_err(|e| miette!("Failed to create dir: {}", e))?;
    for entry in fs::read_dir(src).map_err(|e| miette!("Failed to read dir: {}", e))? {
        let entry = entry.map_err(|e| miette!("Failed to read entry: {}", e))?;
        let path = entry.path();
        let file_name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| miette!("Invalid file name"))?;
        let dst_path = dst.join(file_name);
        if path.is_dir() {
            copy_dir_all(&Utf8PathBuf::from_path_buf(path).map_err(|_| miette!("Invalid path"))?, &dst_path)?;
        } else {
            fs::copy(&path, &dst_path).map_err(|e| miette!("Failed to copy: {}", e))?;
        }
    }
    Ok(())
}
