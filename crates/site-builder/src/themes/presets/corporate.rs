//! Corporate / business theme preset.

use crate::themes::*;

pub fn corporate() -> ThemeSpec {
    ThemeSpec {
        name: "corporate".to_string(),
        colors: ThemeColors {
            primary: "#0F172A".to_string(),
            secondary: "#F8FAFC".to_string(),
            accent: "#3B82F6".to_string(),
            background: "#FFFFFF".to_string(),
            text_primary: "#0F172A".to_string(),
            text_secondary: "#64748B".to_string(),
        },
        typography: ThemeTypography {
            font_primary: "Inter".to_string(),
            font_heading: "Inter".to_string(),
            font_code: Some("JetBrains Mono".to_string()),
            sizes: ThemeFontSizes {
                h1: "48px".to_string(),
                h2: "36px".to_string(),
                h3: "24px".to_string(),
                body: "16px".to_string(),
            },
            weights: ThemeFontWeights {
                regular: 400,
                medium: 500,
                bold: 700,
            },
        },
        spacing: ThemeSpacing {
            base_unit: 8,
            border_radius: "6px".to_string(),
        },
    }
}
