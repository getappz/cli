//! Next.js App Router generator for Lovable React SPA migration.
//!
//! Ports the lovable-nextjs (noxtable) flow to pure Rust. All file I/O uses `ScopedFs`.

use crate::common::{copy_public_assets, copy_tailwind_config, filter_deps};
use crate::types::{MigrationConfig, ProjectAnalysis, RouteInfo, SsgSeverity, SsgWarning};
use camino::Utf8PathBuf;
use miette::{miette, Result};
use regex::Regex;
use sandbox::json_ops;
use sandbox::ScopedFs;
use serde_json::{json, Map, Value};
use std::sync::LazyLock;
use walkdir::WalkDir;

// ── Pre-compiled regexes (compiled once, reused everywhere) ─────────────

static RE_CSS_CHARSET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"@charset\s+[^;]+;").unwrap());
static RE_CSS_IMPORT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"@import\s+(?:url\([^)]*\)|["'][^"']*["'])\s*;"#).unwrap());
static RE_REACT_ROUTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"react-router-dom").unwrap());
static RE_DYNAMIC_PARAM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r":(\w+)").unwrap());
static RE_BROWSER_ROUTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<BrowserRouter[^>]*>[\s\S]*?</BrowserRouter>").unwrap());
static RE_ROUTER_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"import \{[^}]*\} from ["']react-router-dom["'];\s*\n?"#).unwrap()
});
static RE_PAGE_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"import \w+ from ["'][^"']*pages/\w+["'];\s*\n?"#).unwrap()
});
/// Word-boundary match for standalone "App" (function name, export, etc.)
static RE_APP_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bApp\b").unwrap());
/// Detect default image imports: `import varName from "path/to/image.ext"`
static RE_IMAGE_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"import\s+(\w+)\s+from\s+["'][^"']+\.(png|jpe?g|gif|svg|webp|ico|bmp|avif)["']"#)
        .unwrap()
});

// Verification regexes
static RE_NEXT_HEADERS_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"import\s+\{[^}]*\b(cookies|headers)\b[^}]*\}\s+from\s+["']next/headers["']"#)
        .unwrap()
});
static RE_NEXT_CACHE_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"import\s+\{[^}]*\b(revalidatePath|revalidateTag)\b[^}]*\}\s+from\s+["']next/cache["']"#)
        .unwrap()
});
static RE_REDIRECT_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"import\s+\{[^}]*\bredirect\b[^}]*\}\s+from\s+["']next/navigation["']"#)
        .unwrap()
});
static RE_USE_SERVER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?m)^["']use server["'];?"#).unwrap());
static RE_USE_SEARCH_PARAMS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\buseSearchParams\b").unwrap());

// ── Templates ───────────────────────────────────────────────────────────

const USE_ROUTER_TEMPLATE: &str = r#""use client";

import NextLink, { LinkProps } from "next/link";
import { usePathname, useParams, useRouter } from "next/navigation";

const useLocation = () => {
  const pathname = usePathname();
  return { pathname };
};

const useNavigate = () => {
  const router = useRouter();
  return router.push;
};

const Link = ({
  to,
  href,
  ...args
}: Omit<LinkProps, "href"> & {
  to?: string;
  href?: string;
  className?: string;
  children?: React.ReactNode | undefined;
}) => {
  return <NextLink href={href ?? to ?? "/"} {...args} />;
};

export type NavLinkProps = Omit<LinkProps, "href" | "className" | "ref"> & {
  to?: string;
  href?: string;
  ref?: React.Ref<HTMLAnchorElement>;
  className?: string | ((props: { isActive: boolean; isPending: boolean }) => string);
  children?: React.ReactNode;
};

const NavLink = ({
  to,
  href,
  ref,
  className,
  activeClassName,
  pendingClassName,
  children,
  ...args
}: NavLinkProps & {
  activeClassName?: string;
  pendingClassName?: string;
}) => {
  const pathname = usePathname();
  const target = href || to || "/";
  const isActive = pathname === target || (target !== "/" && pathname.startsWith(target + "/"));
  const isPending = false;
  const resolvedClassName =
    typeof className === "function"
      ? className({ isActive, isPending })
      : [className, isActive && activeClassName, isPending && pendingClassName]
          .filter(Boolean)
          .join(" ");
  return (
    <NextLink ref={ref} href={target} className={resolvedClassName} {...args}>
      {children}
    </NextLink>
  );
};

export { Link, NavLink, useLocation, useParams, useNavigate };
"#;

const LAYOUT_TEMPLATE: &str = r#"import "@/index.css";
import Providers from "./providers";

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body>
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}
"#;

const PAGE_TEMPLATE: &str = r#"import PAGENAMEPage from "@/pages/PAGENAME";

export default function PAGENAME() {
  return <PAGENAMEPage />;
}
"#;

// ── Main entry point ────────────────────────────────────────────────────

/// Generate a Next.js App Router project from a Lovable React SPA.
///
/// Returns a list of SSG compatibility warnings (empty when `static_export` is false
/// or the generated project passes all checks).
pub fn generate_nextjs_project(
    config: &MigrationConfig,
    analysis: &ProjectAnalysis,
    fs: &ScopedFs,
) -> Result<Vec<SsgWarning>> {
    let source_dir = &config.source_dir;

    fs.create_dir_all("src/app")?;
    fs.create_dir_all("src/client")?;

    // 1. Copy src → src/client
    let source_src = source_dir.join("src");
    if source_src.exists() {
        fs.copy_from_external(source_src.as_path(), "src/client")
            .map_err(|e| miette!("Failed to copy src: {}", e))?;
    }

    // 2. Single-pass transform: CSS import order, "use client", react-router replacement
    transform_client_files(fs, "src/client")?;

    // 3. Create providers.tsx from App.tsx
    create_providers(fs, source_dir)?;

    // 4. Create layout.tsx
    fs.write_string("src/app/layout.tsx", LAYOUT_TEMPLATE)
        .map_err(|e| miette!("Failed to write layout.tsx: {}", e))?;

    // 5. Create useRouter adapter
    fs.write_string("src/app/useRouter.tsx", USE_ROUTER_TEMPLATE)
        .map_err(|e| miette!("Failed to write useRouter.tsx: {}", e))?;

    // 6. Create App Router pages
    create_app_router_pages(fs, analysis, config.static_export)?;

    // 7. Clean up client files
    cleanup_client_files(fs)?;

    // 8. Merge package.json
    write_package_json(fs, analysis, &config.project_name, config.static_export)?;

    // 9. Copy config files
    copy_config_files(fs, source_dir, config.static_export)?;

    // 10. Copy public
    copy_public_assets(source_dir, fs, "public")?;

    // 11. Verify SSG compatibility (only when static export is enabled)
    let warnings = if config.static_export {
        verify_static_export(fs)?
    } else {
        Vec::new()
    };

    Ok(warnings)
}

// ── Single-pass file transforms ─────────────────────────────────────────

/// Walk `src/client` once and apply all per-file transforms based on extension:
///  - `.css` → fix @import ordering
///  - `.tsx`/`.jsx` → add "use client" + replace react-router-dom
///  - `.ts`/`.js` → replace react-router-dom
fn transform_client_files(fs: &ScopedFs, client_rel: &str) -> Result<()> {
    let root = fs.root();
    let client_path = root.join(client_rel);
    if !client_path.exists() {
        return Ok(());
    }

    for entry in WalkDir::new(&client_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let needs_transform = matches!(ext, "css" | "tsx" | "jsx" | "ts" | "js");
        if !needs_transform {
            continue;
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| miette!("Failed to read {}: {}", path.display(), e))?;

        let new_content = match ext {
            "css" => transform_css(&content),
            "tsx" | "jsx" => {
                let mut c = add_use_client_directive(&content);
                c = replace_react_router(&c);
                c = fix_image_imports(&c);
                Some(c)
            }
            "ts" | "js" => {
                let has_router = RE_REACT_ROUTER.is_match(&content);
                let has_images = RE_IMAGE_IMPORT.is_match(&content);
                if has_router || has_images {
                    let mut c = content.clone();
                    if has_router {
                        c = replace_react_router(&c);
                    }
                    if has_images {
                        c = fix_image_imports(&c);
                    }
                    Some(c)
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(new) = new_content {
            if new != content {
                let rel = path
                    .strip_prefix(root)
                    .map_err(|_| miette!("Path not under root"))?;
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                fs.write_string(&rel_str, &new)
                    .map_err(|e| miette!("Failed to write {}: {}", rel_str, e))?;
            }
        }
    }
    Ok(())
}

/// Move @charset and @import rules to the top of a CSS file.
/// Returns `None` if no reordering is needed.
fn transform_css(content: &str) -> Option<String> {
    let charsets: Vec<String> = RE_CSS_CHARSET
        .find_iter(content)
        .map(|m| m.as_str().to_string())
        .collect();
    let imports: Vec<String> = RE_CSS_IMPORT
        .find_iter(content)
        .map(|m| m.as_str().to_string())
        .collect();

    if charsets.is_empty() && imports.is_empty() {
        return None;
    }

    let mut rest = content.to_string();
    for s in charsets.iter().chain(imports.iter()) {
        rest = rest.replace(s, "");
    }
    while rest.contains("\n\n\n") {
        rest = rest.replace("\n\n\n", "\n\n");
    }
    rest = rest.trim().to_string();

    let header: String = charsets
        .into_iter()
        .chain(imports)
        .map(|s| s + "\n")
        .collect();

    Some(if rest.is_empty() {
        header.trim_end().to_string() + "\n"
    } else {
        format!("{}\n\n{}\n", header.trim_end(), rest)
    })
}

/// Prepend `"use client";` if not already present.
fn add_use_client_directive(content: &str) -> String {
    if content.trim_start().starts_with("\"use client\"")
        || content.trim_start().starts_with("'use client'")
    {
        content.to_string()
    } else {
        format!("\"use client\";\n{}", content)
    }
}

/// Replace `react-router-dom` with `@App/useRouter`.
fn replace_react_router(content: &str) -> String {
    RE_REACT_ROUTER
        .replace_all(content, "@App/useRouter")
        .to_string()
}

/// Fix image imports for Next.js compatibility.
///
/// In Vite, `import img from './hero.jpg'` yields a string URL.
/// In Next.js, the same import yields a `StaticImageData` object.
///
/// Rather than patching every usage site, we rewrite the import itself:
///   `import hero from "./hero.jpg";`
/// becomes:
///   `import _hero_img from "./hero.jpg";`
///   `const hero = _hero_img.src;`
///
/// This keeps the variable as a plain string everywhere it's used (JSX src attrs,
/// object literals, function args, etc.), matching Vite's behaviour.
fn fix_image_imports(content: &str) -> String {
    if !RE_IMAGE_IMPORT.is_match(content) {
        return content.to_string();
    }

    RE_IMAGE_IMPORT
        .replace_all(content, |caps: &regex::Captures| {
            let full_match = caps.get(0).unwrap().as_str();
            let var_name = caps.get(1).unwrap().as_str();
            // Build the private import name
            let private_name = format!("_{var_name}_img");
            // Replace the variable name in the original import statement
            let new_import = full_match.replacen(var_name, &private_name, 1);
            // Add a const that extracts the .src string
            format!("{new_import}\nconst {var_name} = {private_name}.src;")
        })
        .to_string()
}

// ── Providers ───────────────────────────────────────────────────────────

fn create_providers(fs: &ScopedFs, source_dir: &Utf8PathBuf) -> Result<()> {
    let app_path = source_dir.join("src/App.tsx");
    let content = std::fs::read_to_string(app_path.as_path())
        .map_err(|e| miette!("Failed to read App.tsx: {}", e))?;

    // Strip <BrowserRouter>…</BrowserRouter> → {children}
    let mut pc = RE_BROWSER_ROUTER
        .replace_all(&content, "{children}")
        .to_string();

    if !pc.contains("{children}") {
        pc = pc.replace("<Index />", "{children}");
    }

    // Rename: `App = ()` → Providers signature, then standalone `App` → `Providers`
    pc = pc.replace(
        "App = ()",
        "Providers = ({ children }: Readonly<{ children: React.ReactNode }>)",
    );
    // Use word-boundary regex so "AppSidebar" stays "AppSidebar"
    pc = RE_APP_WORD.replace_all(&pc, "Providers").to_string();

    // Fix import paths: only rewrite known src-aliased directories
    let src_dirs = ["./components/", "./pages/", "./lib/", "./hooks/", "./utils/", "./integrations/"];
    for prefix in &src_dirs {
        let replacement = prefix.replacen("./", "@/", 1);
        pc = pc.replace(prefix, &replacement);
    }

    // Remove react-router-dom import
    pc = RE_ROUTER_IMPORT.replace_all(&pc, "").to_string();
    // Remove page component imports (unused in providers)
    pc = RE_PAGE_IMPORT.replace_all(&pc, "").to_string();

    let with_directive = format!("\"use client\";\n{}", pc);
    fs.write_string("src/app/providers.tsx", &with_directive)
        .map_err(|e| miette!("Failed to write providers.tsx: {}", e))?;
    Ok(())
}

// ── App Router pages ────────────────────────────────────────────────────

fn create_app_router_pages(
    fs: &ScopedFs,
    analysis: &ProjectAnalysis,
    static_export: bool,
) -> Result<()> {
    for route in &analysis.routes {
        let (app_segment, page_name, component) = route_to_page_info(route)?;
        let page_path = if app_segment.is_empty() {
            format!("src/app/{}", page_name)
        } else {
            let dir = format!("src/app/{}", app_segment);
            fs.create_dir_all(&dir)
                .map_err(|e| miette!("Failed to create {}: {}", dir, e))?;
            format!("{}/{}", dir, page_name)
        };

        let has_dynamic_params = app_segment.contains('[');
        let page_content = if static_export && has_dynamic_params {
            // Static export requires generateStaticParams for dynamic routes
            let params = extract_dynamic_params(&app_segment);
            let params_fn = build_generate_static_params(&params);
            format!(
                "{}\n\n{}",
                PAGE_TEMPLATE.replace("PAGENAME", &component),
                params_fn
            )
        } else {
            PAGE_TEMPLATE.replace("PAGENAME", &component)
        };

        fs.write_string(&page_path, &page_content)
            .map_err(|e| miette!("Failed to write {}: {}", page_path, e))?;
    }
    Ok(())
}

fn route_to_page_info(route: &RouteInfo) -> Result<(String, String, String)> {
    if route.component == "Index" {
        return Ok(("".to_string(), "page.tsx".to_string(), "Index".to_string()));
    }
    if route.is_catch_all || route.component == "NotFound" {
        return Ok((
            "".to_string(),
            "not-found.tsx".to_string(),
            route.component.clone(),
        ));
    }
    let path = route.path.trim_start_matches('/');
    let segment = RE_DYNAMIC_PARAM.replace_all(path, "[$1]").to_string();
    Ok((segment, "page.tsx".to_string(), route.component.clone()))
}

/// Extract dynamic param names from a segment like `blog/[id]/[slug]` → `["id", "slug"]`
fn extract_dynamic_params(segment: &str) -> Vec<String> {
    let re = Regex::new(r"\[(\w+)\]").unwrap();
    re.captures_iter(segment)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

/// Build a `generateStaticParams` stub for static export.
fn build_generate_static_params(params: &[String]) -> String {
    let obj_fields: Vec<String> = params.iter().map(|p| format!("{p}: \"\"")).collect();
    format!(
        r#"// TODO: Populate with actual param values for static export
export function generateStaticParams() {{
  return [{{ {} }}];
}}"#,
        obj_fields.join(", ")
    )
}

// ── Clean up ────────────────────────────────────────────────────────────

fn cleanup_client_files(fs: &ScopedFs) -> Result<()> {
    let to_remove = ["src/client/main.tsx", "src/client/App.tsx"];
    for rel in &to_remove {
        if fs.exists(rel) {
            fs.remove_file(rel)
                .map_err(|e| miette!("Failed to remove {}: {}", rel, e))?;
        }
    }
    let vite_files = fs
        .glob("src/client/*vite*")
        .map_err(|e| miette!("Failed to glob: {}", e))?;
    for rel in &vite_files {
        if fs.is_file(rel) {
            fs.remove_file(rel)
                .map_err(|e| miette!("Failed to remove {}: {}", rel.display(), e))?;
        }
    }
    Ok(())
}

// ── package.json ────────────────────────────────────────────────────────

fn write_package_json(
    fs: &ScopedFs,
    analysis: &ProjectAnalysis,
    project_name: &str,
    static_export: bool,
) -> Result<()> {
    let mut dependencies = Map::new();
    dependencies.insert("next".to_string(), json!("15.5.6"));
    dependencies.insert("react".to_string(), json!("19.1.0"));
    dependencies.insert("react-dom".to_string(), json!("19.1.0"));

    let filtered = filter_deps(&analysis.dependencies);
    for (dep, version) in &filtered {
        dependencies.insert(dep.clone(), json!(version));
    }

    let mut dev_dependencies = Map::new();
    dev_dependencies.insert("@types/node".to_string(), json!("^20"));
    dev_dependencies.insert("@types/react".to_string(), json!("^19"));
    dev_dependencies.insert("@types/react-dom".to_string(), json!("^19"));
    dev_dependencies.insert("typescript".to_string(), json!("^5"));
    if analysis.has_tailwind {
        // Use the user's version if they had one, otherwise fall back to defaults
        let tw_ver = dependencies.remove("tailwindcss").unwrap_or_else(|| json!("^3.4.0"));
        let pc_ver = dependencies.remove("postcss").unwrap_or_else(|| json!("^8"));
        let ap_ver = dependencies.remove("autoprefixer").unwrap_or_else(|| json!("^10"));
        dev_dependencies.insert("tailwindcss".to_string(), tw_ver);
        dev_dependencies.insert("postcss".to_string(), pc_ver);
        dev_dependencies.insert("autoprefixer".to_string(), ap_ver);
        if !dependencies.contains_key("tailwind-merge") {
            dependencies.insert("tailwind-merge".to_string(), json!("^2.6.0"));
        }
        if !dependencies.contains_key("tailwindcss-animate") {
            dependencies.insert("tailwindcss-animate".to_string(), json!("^1.0.7"));
        }
    }

    let scripts = if static_export {
        json!({
            "dev": "next dev --turbopack",
            "build": "next build",
            "start": "npx serve out"
        })
    } else {
        json!({
            "dev": "next dev --turbopack",
            "build": "next build --turbopack",
            "start": "next start"
        })
    };

    let package: Value = json!({
        "name": project_name,
        "version": "0.1.0",
        "private": true,
        "type": "module",
        "scripts": scripts,
        "dependencies": dependencies,
        "devDependencies": dev_dependencies
    });

    json_ops::write_json_value(fs, "package.json", &package)
        .map_err(|e| miette!("Failed to write package.json: {}", e))?;
    Ok(())
}

// ── Config files ────────────────────────────────────────────────────────

fn copy_config_files(fs: &ScopedFs, source_dir: &Utf8PathBuf, static_export: bool) -> Result<()> {
    let static_lines = if static_export {
        r#"
  output: "export",
  images: { unoptimized: true },"#
    } else {
        ""
    };

    let next_config = format!(
        r#"import type {{ NextConfig }} from "next";
import path from "path";

const nextConfig: NextConfig = {{{static_lines}
  turbopack: {{
    root: path.join(__dirname, "./"),
  }},
}};

export default nextConfig;
"#
    );
    fs.write_string("next.config.ts", &next_config)
        .map_err(|e| miette!("Failed to write next.config.ts: {}", e))?;

    let tsconfig = r#"{
  "compilerOptions": {
    "target": "ES2017",
    "lib": ["dom", "dom.iterable", "esnext"],
    "allowJs": true,
    "skipLibCheck": true,
    "strict": true,
    "noEmit": true,
    "esModuleInterop": true,
    "module": "esnext",
    "moduleResolution": "bundler",
    "resolveJsonModule": true,
    "isolatedModules": true,
    "jsx": "preserve",
    "incremental": true,
    "plugins": [{ "name": "next" }],
    "paths": {
      "@/*": ["./src/client/*"],
      "@App/*": ["./src/app/*"]
    }
  },
  "include": ["next-env.d.ts", "**/*.ts", "**/*.tsx", ".next/types/**/*.ts"],
  "exclude": ["node_modules"]
}
"#;
    fs.write_string("tsconfig.json", tsconfig)
        .map_err(|e| miette!("Failed to write tsconfig.json: {}", e))?;

    copy_tailwind_config(source_dir, fs)?;

    let postcss_ts = source_dir.join("postcss.config.ts");
    if postcss_ts.exists() {
        fs.copy_from_external(postcss_ts.as_path(), "postcss.config.ts")
            .map_err(|e| miette!("Failed to copy postcss.config.ts: {}", e))?;
    }
    let postcss_js = source_dir.join("postcss.config.js");
    if postcss_js.exists() {
        fs.copy_from_external(postcss_js.as_path(), "postcss.config.js")
            .map_err(|e| miette!("Failed to copy postcss.config.js: {}", e))?;
    }

    let components_json = source_dir.join("components.json");
    if components_json.exists() {
        let content = std::fs::read_to_string(components_json.as_path())
            .map_err(|e| miette!("Failed to read components.json: {}", e))?;
        let rewritten = content.replace("src/", "src/client/");
        fs.write_string("components.json", &rewritten)
            .map_err(|e| miette!("Failed to write components.json: {}", e))?;
    }
    Ok(())
}

// ── SSG verification ────────────────────────────────────────────────────

/// Verify that the generated Next.js project is compatible with static export.
///
/// Runs five checks against the generated files and returns a list of warnings/errors.
fn verify_static_export(fs: &ScopedFs) -> Result<Vec<SsgWarning>> {
    let mut warnings: Vec<SsgWarning> = Vec::new();
    let root = fs.root();

    // ── Check 1: next.config.ts has output:"export" and images:{unoptimized:true} ──
    let config_path = root.join("next.config.ts");
    if config_path.exists() {
        let config_content = std::fs::read_to_string(&config_path)
            .map_err(|e| miette!("Failed to read next.config.ts: {}", e))?;
        if !config_content.contains("output:") || !config_content.contains("export") {
            warnings.push(SsgWarning {
                severity: SsgSeverity::Error,
                message: "next.config.ts is missing `output: \"export\"` — static build will produce SSR output instead of static HTML".to_string(),
                file: Some("next.config.ts".to_string()),
            });
        }
        if !config_content.contains("unoptimized") {
            warnings.push(SsgWarning {
                severity: SsgSeverity::Error,
                message: "next.config.ts is missing `images: { unoptimized: true }` — image optimization requires a Node server and is incompatible with static export".to_string(),
                file: Some("next.config.ts".to_string()),
            });
        }
    } else {
        warnings.push(SsgWarning {
            severity: SsgSeverity::Error,
            message: "next.config.ts not found — cannot verify static export configuration".to_string(),
            file: None,
        });
    }

    // ── Check 2: Dynamic route pages have generateStaticParams ──
    let app_dir = root.join("src/app");
    if app_dir.exists() {
        for entry in WalkDir::new(&app_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name != "page.tsx" {
                continue;
            }
            // Check if any ancestor directory is a dynamic segment [param]
            let rel = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            if !rel.contains('[') {
                continue;
            }
            let content = std::fs::read_to_string(path)
                .map_err(|e| miette!("Failed to read {}: {}", rel, e))?;
            if !content.contains("generateStaticParams") {
                warnings.push(SsgWarning {
                    severity: SsgSeverity::Error,
                    message: format!(
                        "Dynamic route page is missing `generateStaticParams()` — static export requires this to know which paths to pre-render"
                    ),
                    file: Some(rel),
                });
            }
        }
    }

    // ── Checks 3-5: Scan all src/ files in one pass ──
    let src_dir = root.join("src");
    if src_dir.exists() {
        for entry in WalkDir::new(&src_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !matches!(ext, "ts" | "tsx" | "jsx" | "js") {
                continue;
            }
            let rel = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            // Check 3: Unsupported server APIs
            if RE_NEXT_HEADERS_IMPORT.is_match(&content) {
                let cap = RE_NEXT_HEADERS_IMPORT.captures(&content).unwrap();
                let api = cap.get(1).map(|m| m.as_str()).unwrap_or("cookies/headers");
                warnings.push(SsgWarning {
                    severity: SsgSeverity::Warning,
                    message: format!(
                        "Imports `{api}` from `next/headers` — this server-only API is not available in static export"
                    ),
                    file: Some(rel.clone()),
                });
            }
            if RE_NEXT_CACHE_IMPORT.is_match(&content) {
                let cap = RE_NEXT_CACHE_IMPORT.captures(&content).unwrap();
                let api = cap.get(1).map(|m| m.as_str()).unwrap_or("revalidatePath/revalidateTag");
                warnings.push(SsgWarning {
                    severity: SsgSeverity::Warning,
                    message: format!(
                        "Imports `{api}` from `next/cache` — revalidation is not available in static export"
                    ),
                    file: Some(rel.clone()),
                });
            }
            if RE_REDIRECT_IMPORT.is_match(&content) {
                warnings.push(SsgWarning {
                    severity: SsgSeverity::Warning,
                    message: "Imports `redirect` from `next/navigation` — server-side redirect is not available in static export; use client-side `useRouter().push()` instead".to_string(),
                    file: Some(rel.clone()),
                });
            }
            if RE_USE_SERVER.is_match(&content) {
                warnings.push(SsgWarning {
                    severity: SsgSeverity::Warning,
                    message: "Contains `\"use server\"` directive — server actions are not available in static export".to_string(),
                    file: Some(rel.clone()),
                });
            }

            // Check 4: Residual raw image imports (transform should have rewritten them)
            // After the transform, image imports look like `import _var_img from "..."`
            // If we still see raw `import var from "...ext"`, the transform was missed.
            if RE_IMAGE_IMPORT.is_match(&content) {
                let raw_vars: Vec<String> = RE_IMAGE_IMPORT
                    .captures_iter(&content)
                    .filter_map(|cap| {
                        let var = cap.get(1)?.as_str();
                        // If the variable already starts with _ and ends with _img, it was transformed
                        if var.starts_with('_') && var.ends_with("_img") {
                            None
                        } else {
                            Some(var.to_string())
                        }
                    })
                    .collect();
                for var in &raw_vars {
                    warnings.push(SsgWarning {
                        severity: SsgSeverity::Warning,
                        message: format!(
                            "Image import `{var}` was not rewritten — Next.js image imports return StaticImageData (not a string). Re-run migration or manually add `.src` where `{var}` is used"
                        ),
                        file: Some(rel.clone()),
                    });
                }
            }

            // Check 5: useSearchParams without Suspense
            if RE_USE_SEARCH_PARAMS.is_match(&content) && !content.contains("Suspense") {
                warnings.push(SsgWarning {
                    severity: SsgSeverity::Warning,
                    message: "Uses `useSearchParams` without a `<Suspense>` boundary — static export requires wrapping useSearchParams in Suspense to avoid build errors".to_string(),
                    file: Some(rel.clone()),
                });
            }
        }
    }

    Ok(warnings)
}
