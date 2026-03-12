//! Minimal / default theme preset.

use crate::themes::*;

pub fn minimal() -> ThemeSpec {
    ThemeSpec {
        name: "minimal".to_string(),
        colors: ThemeColors {
            primary: "#3B82F6".to_string(),
            secondary: "#F1F5F9".to_string(),
            accent: "#F59E0B".to_string(),
            background: "#FFFFFF".to_string(),
            text_primary: "#1F2937".to_string(),
            text_secondary: "#6B7280".to_string(),
        },
        typography: ThemeTypography {
            font_primary: "Inter".to_string(),
            font_heading: "Inter".to_string(),
            font_code: None,
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
            border_radius: "8px".to_string(),
        },
    }
}
