//! Layout generation for Astro projects.

use crate::vfs::Vfs;
use camino::Utf8PathBuf;
use miette::{miette, Result};

pub(super) fn generate_layout(
    vfs: &dyn Vfs,
    layouts_dir: &Utf8PathBuf,
    src_dir: &Utf8PathBuf,
) -> Result<()> {
    let layout_path = layouts_dir.join("Layout.astro");

    let mut css_imports = String::new();
    for css_file in ["index.css", "App.css"] {
        if vfs.exists(src_dir.join(css_file).as_str()) {
            css_imports.push_str(&format!("import '../{}';\n", css_file));
        }
    }

    let layout = format!(
        r#"---
interface Props {{
  title?: string;
}}
const {{ title = "Migrated Astro App" }} = Astro.props;
{}
---
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{{title}}</title>
  </head>
  <body>
    <slot />
  </body>
</html>
"#,
        css_imports
    );

    vfs.write_string(layout_path.as_str(), &layout)
        .map_err(|e| miette!("Failed to write Layout.astro: {}", e))?;
    Ok(())
}
