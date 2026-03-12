//! File copying helpers for Astro project generation.

use crate::vfs::Vfs;
use camino::Utf8PathBuf;
use miette::{miette, Result};

pub(super) fn copy_tailwind_config(
    vfs: &dyn Vfs,
    source_dir: &Utf8PathBuf,
    output_dir: &Utf8PathBuf,
) -> Result<()> {
    for ext in ["ts", "js"] {
        let src = source_dir.join(format!("tailwind.config.{ext}"));
        if vfs.exists(src.as_str()) {
            let dst = output_dir.join(format!("tailwind.config.{ext}"));
            vfs.copy_file(src.as_str(), dst.as_str())
                .map_err(|e| miette!("Failed to copy tailwind.config.{}: {}", ext, e))?;
        }
    }
    Ok(())
}

pub(super) fn copy_public_assets(
    vfs: &dyn Vfs,
    source_dir: &Utf8PathBuf,
    public_dir: &Utf8PathBuf,
) -> Result<()> {
    let source_public = source_dir.join("public");
    if vfs.exists(source_public.as_str()) {
        copy_dir_all(vfs, &source_public, public_dir)?;
    }
    Ok(())
}

pub(super) fn copy_css_files(
    vfs: &dyn Vfs,
    source_src_dir: &Utf8PathBuf,
    dest_src_dir: &Utf8PathBuf,
) -> Result<()> {
    if !vfs.exists(source_src_dir.as_str()) {
        return Ok(());
    }
    for entry in vfs.list_dir(source_src_dir.as_str())? {
        if entry.is_file {
            let path = Utf8PathBuf::from(&entry.path);
            if path.extension() == Some("css") {
                if let Some(file_name) = path.file_name() {
                    let dest_path = dest_src_dir.join(file_name);
                    vfs.copy_file(&entry.path, dest_path.as_str())
                        .map_err(|e| miette!("Failed to copy CSS {}: {}", file_name, e))?;
                }
            }
        }
    }
    Ok(())
}

pub(super) fn copy_dir_all(
    vfs: &dyn Vfs,
    src: &Utf8PathBuf,
    dst: &Utf8PathBuf,
) -> Result<()> {
    vfs.copy_dir(src.as_str(), dst.as_str())
        .map_err(|e| miette!("Failed to copy directory: {}", e))
}
