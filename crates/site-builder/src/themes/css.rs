//! Generate global.css with CSS custom properties from a ThemeSpec.

use super::ThemeSpec;

/// Generate the content of a `global.css` file with CSS custom properties
/// and Google Fonts imports.
pub fn generate_global_css(theme: &ThemeSpec) -> String {
    let mut fonts_to_import = vec![&theme.typography.font_primary];
    if theme.typography.font_heading != theme.typography.font_primary {
        fonts_to_import.push(&theme.typography.font_heading);
    }
    if let Some(ref code_font) = theme.typography.font_code {
        fonts_to_import.push(code_font);
    }

    // Build Google Fonts import URL
    let font_params: Vec<String> = fonts_to_import
        .iter()
        .map(|f| {
            let encoded = f.replace(' ', "+");
            format!("family={}:wght@{};{};{}", encoded, theme.typography.weights.regular, theme.typography.weights.medium, theme.typography.weights.bold)
        })
        .collect();

    let fonts_url = format!(
        "https://fonts.googleapis.com/css2?{}&display=swap",
        font_params.join("&")
    );

    format!(
        r#"@import url('{fonts_url}');

@tailwind base;
@tailwind components;
@tailwind utilities;

:root {{
  /* Colors */
  --color-primary: {primary};
  --color-secondary: {secondary};
  --color-accent: {accent};
  --color-background: {background};
  --color-text-primary: {text_primary};
  --color-text-secondary: {text_secondary};

  /* Spacing */
  --spacing-base: {base_unit}px;
  --radius: {border_radius};
}}

*,
*::before,
*::after {{
  transition-property: color, background-color, border-color, box-shadow, transform, opacity;
  transition-timing-function: cubic-bezier(0.4, 0, 0.2, 1);
  transition-duration: 150ms;
}}

html {{
  scroll-behavior: smooth;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}}

body {{
  font-family: '{font_primary}', ui-sans-serif, system-ui, sans-serif;
  background-color: var(--color-background);
  color: var(--color-text-primary);
  line-height: 1.7;
  font-size: 16px;
}}

h1, h2, h3, h4, h5, h6 {{
  font-family: '{font_heading}', ui-sans-serif, system-ui, sans-serif;
  font-weight: {bold};
  line-height: 1.2;
  letter-spacing: -0.02em;
}}

/* Ensure images are responsive by default */
img {{
  max-width: 100%;
  height: auto;
}}

/* Better focus styles for accessibility */
:focus-visible {{
  outline: 2px solid var(--color-primary);
  outline-offset: 2px;
  border-radius: var(--radius);
}}

/* Prose overrides for markdown content */
.prose {{
  --tw-prose-headings: var(--color-text-primary);
  --tw-prose-body: var(--color-text-secondary);
  --tw-prose-links: var(--color-primary);
}}

.prose p {{
  margin-bottom: 1.25em;
}}

.prose h2 {{
  margin-top: 2em;
  margin-bottom: 0.75em;
}}

.prose h3 {{
  margin-top: 1.5em;
  margin-bottom: 0.5em;
}}
"#,
        primary = theme.colors.primary,
        secondary = theme.colors.secondary,
        accent = theme.colors.accent,
        background = theme.colors.background,
        text_primary = theme.colors.text_primary,
        text_secondary = theme.colors.text_secondary,
        base_unit = theme.spacing.base_unit,
        border_radius = theme.spacing.border_radius,
        font_primary = theme.typography.font_primary,
        font_heading = theme.typography.font_heading,
        bold = theme.typography.weights.bold,
    )
}
