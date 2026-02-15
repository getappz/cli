//! Layout generation for Astro projects.

use camino::Utf8PathBuf;
use miette::{miette, Result};
use std::fs;
use std::io::Write;

pub(super) fn generate_layout(layouts_dir: &Utf8PathBuf) -> Result<()> {
    let layout_path = layouts_dir.join("Layout.astro");
    let mut file = fs::File::create(&layout_path)
        .map_err(|e| miette!("Failed to create Layout.astro: {}", e))?;

    let src_dir = layouts_dir
        .parent()
        .ok_or_else(|| miette!("Invalid layout directory"))?;
    let mut css_imports = String::new();
    for css_file in ["index.css", "App.css"] {
        if src_dir.join(css_file).exists() {
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

    file.write_all(layout.as_bytes())
        .map_err(|e| miette!("Failed to write Layout.astro: {}", e))?;
    Ok(())
}
