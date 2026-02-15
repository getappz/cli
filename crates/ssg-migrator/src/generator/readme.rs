//! README generation for migrated Astro projects.

use camino::Utf8PathBuf;
use miette::{miette, Result};
use std::fs;
use std::io::Write;

pub(super) fn generate_readme(output_dir: &Utf8PathBuf, project_name: &str) -> Result<()> {
    let readme_path = output_dir.join("README.md");
    let mut file = fs::File::create(&readme_path)
        .map_err(|e| miette!("Failed to create README.md: {}", e))?;

    let readme = format!(
        r#"# {}

This project was migrated from a React SPA to Astro using the SSG migrator.

## Getting Started

1. Install dependencies:
```bash
npm install
```

2. Start the development server:
```bash
npm run dev
```

3. Build for production:
```bash
npm run build
```

## Migration Notes

- React components that use hooks or browser APIs are kept as React components in `src/components/ui/`
- Static components have been converted to Astro components
- Routes have been converted to Astro pages in `src/pages/`
"#,
        project_name
    );

    file.write_all(readme.as_bytes())
        .map_err(|e| miette!("Failed to write README.md: {}", e))?;
    Ok(())
}
