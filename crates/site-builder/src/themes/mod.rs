//! Theme system: dual-source (extracted from Firecrawl branding + pre-built presets).

pub mod css;
pub mod presets;
pub mod tailwind;

use crate::firecrawl::types::BrandingData;
use serde::{Deserialize, Serialize};

/// A complete theme specification for generating Tailwind config and CSS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeSpec {
    pub name: String,
    pub colors: ThemeColors,
    pub typography: ThemeTypography,
    pub spacing: ThemeSpacing,
}

/// Color palette for a theme.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeColors {
    pub primary: String,
    pub secondary: String,
    pub accent: String,
    pub background: String,
    pub text_primary: String,
    pub text_secondary: String,
}

/// Typography settings for a theme.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeTypography {
    pub font_primary: String,
    pub font_heading: String,
    pub font_code: Option<String>,
    pub sizes: ThemeFontSizes,
    pub weights: ThemeFontWeights,
}

/// Font sizes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeFontSizes {
    pub h1: String,
    pub h2: String,
    pub h3: String,
    pub body: String,
}

/// Font weights.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeFontWeights {
    pub regular: u32,
    pub medium: u32,
    pub bold: u32,
}

/// Spacing settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeSpacing {
    pub base_unit: u32,
    pub border_radius: String,
}

impl ThemeSpec {
    /// Create a ThemeSpec from Firecrawl branding data.
    pub fn from_branding(branding: &BrandingData) -> Self {
        Self {
            name: "extracted".to_string(),
            colors: ThemeColors {
                primary: branding.colors.primary.clone(),
                secondary: branding.colors.secondary.clone(),
                accent: branding.colors.accent.clone(),
                background: branding.colors.background.clone(),
                text_primary: branding.colors.text_primary.clone(),
                text_secondary: branding.colors.text_secondary.clone(),
            },
            typography: ThemeTypography {
                font_primary: branding.typography.font_families.primary.clone(),
                font_heading: branding.typography.font_families.heading.clone(),
                font_code: branding.typography.font_families.code.clone(),
                sizes: ThemeFontSizes {
                    h1: branding.typography.font_sizes.h1.clone(),
                    h2: branding.typography.font_sizes.h2.clone(),
                    h3: branding.typography.font_sizes.h3.clone(),
                    body: branding.typography.font_sizes.body.clone(),
                },
                weights: ThemeFontWeights {
                    regular: branding.typography.font_weights.regular,
                    medium: branding.typography.font_weights.medium,
                    bold: branding.typography.font_weights.bold,
                },
            },
            spacing: ThemeSpacing {
                base_unit: branding.spacing.base_unit,
                border_radius: branding.spacing.border_radius.clone(),
            },
        }
    }
}

/// Look up a pre-built theme by name, or return the default minimal theme.
pub fn get_preset_theme(name: &str) -> ThemeSpec {
    match name {
        "nonprofit" => presets::nonprofit(),
        "corporate" => presets::corporate(),
        "startup" => presets::startup(),
        _ => presets::minimal(),
    }
}

/// Resolve a theme from available sources.
///
/// Priority: CLI override > branding extraction > AI suggestion > minimal fallback.
pub fn resolve_theme(
    theme_override: Option<&str>,
    branding: Option<&BrandingData>,
    suggested_theme: Option<&str>,
) -> ThemeSpec {
    // 1. CLI override
    if let Some(name) = theme_override {
        return get_preset_theme(name);
    }

    // 2. Extracted from branding data
    if let Some(branding) = branding {
        return ThemeSpec::from_branding(branding);
    }

    // 3. AI-suggested theme
    if let Some(name) = suggested_theme {
        return get_preset_theme(name);
    }

    // 4. Fallback
    presets::minimal()
}
