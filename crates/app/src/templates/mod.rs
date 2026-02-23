/// Built-in templates for popular static site generators
pub const BUILTIN_TEMPLATES: &[(&str, &str, &str, Option<&str>)] = &[
    // JavaScript/TypeScript frameworks
    (
        "nextjs",
        "Next.js",
        "vercel/next.js",
        Some("examples/hello-world"),
    ),
    (
        "astro",
        "Astro",
        "withastro/astro",
        Some("examples/starter"),
    ),
    ("gatsby", "Gatsby", "gatsbyjs/gatsby-starter-default", None),
    ("hugo", "Hugo", "gohugoio/hugo", Some("exampleSite")),
    ("jekyll", "Jekyll", "jekyll/jekyll", Some("site")),
    ("nuxt", "Nuxt", "nuxt/starter", None),
    (
        "vite",
        "Vite",
        "vitejs/vite",
        Some("packages/create-vite/template-vanilla"),
    ),
    (
        "sveltekit",
        "SvelteKit",
        "sveltejs/kit",
        Some("packages/create-svelte/templates/default"),
    ),
    ("remix", "Remix", "remix-run/remix", Some("templates/remix")),
    (
        "eleventy",
        "Eleventy",
        "11ty/eleventy",
        Some("examples/basic"),
    ),
    // Documentation SSGs
    (
        "docusaurus",
        "Docusaurus",
        "facebook/docusaurus",
        Some("website"),
    ),
    ("vitepress", "VitePress", "vuejs/vitepress", None),
    ("nextra", "Nextra", "shuding/nextra", Some("examples/basic")),
    // PHP-based static site generators
    (
        "wordpress",
        "WordPress",
        "wordpress.org",
        None,
    ),
    ("sculpin", "Sculpin", "sculpin/sculpin", Some("skeleton")),
    ("spress", "Spress", "spress/Spress", Some("skeleton")),
    ("kirby", "Kirby", "getkirby/starterkit", None),
    ("statamic", "Statamic", "statamic/starter-kit", None),
    // Additional static site generators
    ("hexo", "Hexo", "hexojs/hexo-starter", None),
    ("zola", "Zola", "ekzhang/zola-blog-starter", None),
    (
        "pelican",
        "Pelican",
        "getpelican/pelican",
        Some("samples/basic"),
    ),
    (
        "mkdocs",
        "MkDocs",
        "sosiristseng/template-mkdocs-material",
        None,
    ),
    ("mdbook", "mdBook", "MichaelCurrin/mdbook-quickstart", None),
    (
        "middleman",
        "Middleman",
        "middleman/middleman-templates-default",
        Some("template"),
    ),
    ("jigsaw", "Jigsaw", "tighten/jigsaw-docs-template", None),
];

/// Get built-in template by name
pub fn get_builtin_template(name: &str) -> Option<(&str, Option<&str>)> {
    BUILTIN_TEMPLATES
        .iter()
        .find(|(slug, _, _, _)| *slug == name)
        .map(|(_, _, repo, subfolder)| (*repo, *subfolder))
}
