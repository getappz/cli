//! Startup / tech theme preset.

use crate::themes::*;

pub fn startup() -> ThemeSpec {
    ThemeSpec {
        name: "startup".to_string(),
        colors: ThemeColors {
            primary: "#7C3AED".to_string(),
            secondary: "#F5F3FF".to_string(),
            accent: "#F59E0B".to_string(),
            background: "#FFFFFF".to_string(),
            text_primary: "#111827".to_string(),
            text_secondary: "#6B7280".to_string(),
        },
        typography: ThemeTypography {
            font_primary: "Inter".to_string(),
            font_heading: "Plus Jakarta Sans".to_string(),
            font_code: Some("Fira Code".to_string()),
            sizes: ThemeFontSizes {
                h1: "56px".to_string(),
                h2: "40px".to_string(),
                h3: "28px".to_string(),
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
            border_radius: "12px".to_string(),
        },
    }
}
