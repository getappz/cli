//! Embedded Astro component and layout templates.
//!
//! All templates use CSS custom properties so they adapt to any theme.
//! Components are deterministic — AI never generates component code,
//! it only selects which components to use and what content to populate.

/// Base layout template (includes nav + footer shell).
pub const BASE_LAYOUT: &str = include_str!("base_layout.astro");

/// Astro config template.
pub const ASTRO_CONFIG: &str = r#"import { defineConfig } from 'astro/config';
import tailwind from '@astrojs/tailwind';
import mdx from '@astrojs/mdx';

export default defineConfig({
  integrations: [tailwind(), mdx()],
});
"#;

/// Content collection config template.
pub const CONTENT_CONFIG: &str = r#"import { defineCollection, z } from 'astro:content';

const pages = defineCollection({
  type: 'content',
  schema: z.object({
    title: z.string(),
    description: z.string().optional(),
    layout: z.string().default('default'),
  }),
});

export const collections = { pages };
"#;

/// Embedded component templates.
pub mod components {
    pub const HERO: &str = include_str!("components/hero.astro");
    pub const SECTION: &str = include_str!("components/section.astro");
    pub const CARD_GRID: &str = include_str!("components/card_grid.astro");
    pub const CTA: &str = include_str!("components/cta.astro");
    pub const NAVBAR: &str = include_str!("components/navbar.astro");
    pub const FOOTER: &str = include_str!("components/footer.astro");
    pub const STATS: &str = include_str!("components/stats.astro");
    pub const TESTIMONIAL: &str = include_str!("components/testimonial.astro");
}
