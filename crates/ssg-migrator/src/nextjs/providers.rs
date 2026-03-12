//! Providers creation from App.tsx.

use super::regex::{
    RE_APP_WORD, RE_BROWSER_ROUTER, RE_PAGE_IMPORT, RE_ROUTER_IMPORT,
};
use crate::vfs::Vfs;
use camino::Utf8PathBuf;
use miette::{miette, Result};

pub(super) fn create_providers(
    vfs: &dyn Vfs,
    output_dir: &Utf8PathBuf,
    source_dir: &Utf8PathBuf,
) -> Result<()> {
    let app_path = if vfs.exists(source_dir.join("src/App.tsx").as_str()) {
        source_dir.join("src/App.tsx")
    } else if vfs.exists(source_dir.join("src/App.jsx").as_str()) {
        source_dir.join("src/App.jsx")
    } else {
        return Err(miette!("Neither src/App.tsx nor src/App.jsx found"));
    };
    let content = vfs
        .read_to_string(app_path.as_str())
        .map_err(|e| miette!("Failed to read App: {}", e))?;

    let mut pc = RE_BROWSER_ROUTER.replace_all(&content, "{children}").to_string();

    if !pc.contains("{children}") {
        pc = pc.replace("<Index />", "{children}");
    }

    pc = pc.replace(
        "App = ()",
        "Providers = ({ children }: Readonly<{ children: React.ReactNode }>)",
    );
    pc = RE_APP_WORD.replace_all(&pc, "Providers").to_string();

    let src_dirs = ["./components/", "./pages/", "./lib/", "./hooks/", "./utils/", "./integrations/"];
    for prefix in &src_dirs {
        let replacement = prefix.replacen("./", "@/", 1);
        pc = pc.replace(prefix, &replacement);
    }

    pc = RE_ROUTER_IMPORT.replace_all(&pc, "").to_string();
    pc = RE_PAGE_IMPORT.replace_all(&pc, "").to_string();

    let with_directive = format!("\"use client\";\n{}", pc);
    vfs.write_string(
        output_dir.join("src/app/providers.tsx").as_str(),
        &with_directive,
    )
    .map_err(|e| miette!("Failed to write providers.tsx: {}", e))?;
    Ok(())
}
