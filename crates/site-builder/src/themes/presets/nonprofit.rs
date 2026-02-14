//! Nonprofit / NGO theme preset.

use crate::themes::*;

pub fn nonprofit() -> ThemeSpec {
    ThemeSpec {
        name: "nonprofit".to_string(),
        colors: ThemeColors {
            primary: "#1F4E79".to_string(),
            secondary: "#F4F7FA".to_string(),
            accent: "#2E8B57".to_string(),
            background: "#FFFFFF".to_string(),
            text_primary: "#1F2937".to_string(),
            text_secondary: "#6B7280".to_string(),
        },
        typography: ThemeTypography {
            font_primary: "Source Sans Pro".to_string(),
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
