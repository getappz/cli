//! Generate tailwind.config.mjs from a ThemeSpec.

use super::ThemeSpec;

/// Generate the content of a `tailwind.config.mjs` file from a theme.
pub fn generate_tailwind_config(theme: &ThemeSpec) -> String {
    let code_font = theme
        .typography
        .font_code
        .as_deref()
        .map(|f| format!(r#"        mono: ['{f}', 'ui-monospace', 'monospace'],"#))
        .unwrap_or_default();

    format!(
        r#"/** @type {{import('tailwindcss').Config}} */
export default {{
  content: ['./src/**/*.{{astro,html,js,jsx,md,mdx,ts,tsx}}'],
  theme: {{
    extend: {{
      colors: {{
        primary: 'var(--color-primary)',
        secondary: 'var(--color-secondary)',
        accent: 'var(--color-accent)',
        background: 'var(--color-background)',
        'text-primary': 'var(--color-text-primary)',
        'text-secondary': 'var(--color-text-secondary)',
      }},
      fontFamily: {{
        sans: ['{font_primary}', 'ui-sans-serif', 'system-ui', 'sans-serif'],
        heading: ['{font_heading}', 'ui-sans-serif', 'system-ui', 'sans-serif'],
{code_font}
      }},
      borderRadius: {{
        DEFAULT: 'var(--radius)',
      }},
    }},
  }},
  plugins: [
    require('@tailwindcss/typography'),
  ],
}};
"#,
        font_primary = theme.typography.font_primary,
        font_heading = theme.typography.font_heading,
    )
}
