//! Providers creation from App.tsx.

use super::regex::{
    RE_APP_WORD, RE_BROWSER_ROUTER, RE_PAGE_IMPORT, RE_ROUTER_IMPORT,
};
use camino::Utf8PathBuf;
use miette::{miette, Result};
use sandbox::ScopedFs;

pub(super) fn create_providers(fs: &ScopedFs, source_dir: &Utf8PathBuf) -> Result<()> {
    let app_path = source_dir.join("src/App.tsx");
    let content = std::fs::read_to_string(app_path.as_path())
        .map_err(|e| miette!("Failed to read App.tsx: {}", e))?;

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
    fs.write_string("src/app/providers.tsx", &with_directive)
        .map_err(|e| miette!("Failed to write providers.tsx: {}", e))?;
    Ok(())
}
