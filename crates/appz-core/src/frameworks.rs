use serde::Serialize;

/// Framework detected from project dependencies.
#[derive(Debug, Clone, Serialize)]
pub struct DetectedFramework {
    pub name: &'static str,
    pub ecosystem: &'static str,
}

/// Known frameworks per ecosystem, keyed by npm package / cargo crate / etc.
const NPM_FRAMEWORKS: &[(&[&str], &str)] = &[
    (&["next", "next/dist/server/next-server"], "Next.js"),
    (&["react", "react-dom"], "React"),
    (&["vue", "@vue/core"], "Vue"),
    (&["@angular/core"], "Angular"),
    (&["svelte", "@sveltejs/kit"], "Svelte"),
    (&["nuxt", "nuxt3"], "Nuxt"),
    (&["gatsby"], "Gatsby"),
    (&["remix", "@remix-run/react"], "Remix"),
    (&["astro"], "Astro"),
    // Server-rendered variants, told apart by the adapter / SSR package they
    // depend on: a static Astro or SvelteKit project has none of these.
    (
        &[
            "@astrojs/node",
            "@astrojs/vercel",
            "@astrojs/netlify",
            "@astrojs/cloudflare",
        ],
        "Astro SSR",
    ),
    (
        &[
            "@sveltejs/adapter-auto",
            "@sveltejs/adapter-node",
            "@sveltejs/adapter-vercel",
            "@sveltejs/adapter-netlify",
            "@sveltejs/adapter-cloudflare",
        ],
        "SvelteKit SSR",
    ),
    (&["@solidjs/start", "solid-start"], "SolidStart"),
    (&["@builder.io/qwik-city"], "Qwik City"),
    (&["@angular/ssr", "@angular/platform-server"], "Angular SSR"),
    (&["solid-js", "solid-start"], "SolidJS"),
    (&["@nestjs/core"], "NestJS"),
    (&["express"], "Express"),
    (&["fastify"], "Fastify"),
    (&["hono"], "Hono"),
    (&["elysia"], "Elysia"),
    (&["prisma"], "Prisma"),
    (&["drizzle-orm"], "Drizzle"),
];

/// Detected frameworks that render on the server: a platform builds these into
/// functions with its own build, so a separate static build would be wasted.
/// Frameworks that can also be static (Astro, SvelteKit, Angular) are listed
/// only as their "SSR" variant, detected from the adapter dependency.
pub const SERVER_RENDERED_FRAMEWORKS: &[&str] = &[
    "Next.js",
    "Nuxt",
    "Remix",
    "Astro SSR",
    "SvelteKit SSR",
    "SolidStart",
    "Qwik City",
    "Angular SSR",
];

const CARGO_FRAMEWORKS: &[(&[&str], &str)] = &[
    (&["axum"], "Axum"),
    (&["actix-web"], "Actix"),
    (&["rocket"], "Rocket"),
    (&["tide"], "Tide"),
    (&["warp"], "Warp"),
    (&["salvo"], "Salvo"),
    (&["poem"], "Poem"),
    (&["leptos"], "Leptos"),
    (&["yew"], "Yew"),
    (&["dioxus"], "Dioxus"),
    (&["tauri"], "Tauri"),
    (&["sqlx"], "SQLx"),
    (&["diesel"], "Diesel"),
    (&["sea-orm"], "SeaORM"),
];

const PYTHON_FRAMEWORKS: &[(&[&str], &str)] = &[
    (&["django"], "Django"),
    (&["flask"], "Flask"),
    (&["fastapi"], "FastAPI"),
    (&["starlette"], "Starlette"),
    (&["tornado"], "Tornado"),
    (&["aiohttp"], "aiohttp"),
    (&["litestar"], "Litestar"),
    (&["sqlalchemy"], "SQLAlchemy"),
    (&["pydantic"], "Pydantic"),
];

// ── Go ─────────────────────────────────────────────────

const GO_FRAMEWORKS: &[(&[&str], &str)] = &[
    (&["github.com/gin-gonic/gin"], "Gin"),
    (&["github.com/labstack/echo"], "Echo"),
    (&["github.com/go-chi/chi"], "Chi"),
    (&["github.com/gofiber/fiber"], "Fiber"),
    (&["github.com/gobuffalo/buffalo"], "Buffalo"),
    (&["github.com/gorilla/mux"], "Gorilla Mux"),
    (&["github.com/a-h/templ"], "Templ"),
    (&["github.com/grpc/grpc-go"], "gRPC-Go"),
    (&["github.com/urfave/cli"], "CLI"),
    (&["github.com/spf13/cobra"], "Cobra"),
    (&["github.com/gin-gonic/contrib"], "Gin Contrib"),
];

// ── Ruby ────────────────────────────────────────────────

const RUBY_FRAMEWORKS: &[(&[&str], &str)] = &[
    (&["rails", "railties"], "Ruby on Rails"),
    (&["sinatra"], "Sinatra"),
    (&["hanami"], "Hanami"),
    (&["roda"], "Roda"),
    (&["grape"], "Grape"),
    (&["rack"], "Rack"),
    (&["jekyll"], "Jekyll"),
    (&["middleman"], "Middleman"),
    (&["padrino"], "Padrino"),
];

// ── Java/JVM ────────────────────────────────────────────

const JAVA_FRAMEWORKS: &[(&[&str], &str)] = &[
    (&["spring-boot", "spring-boot-starter"], "Spring Boot"),
    (&["quarkus"], "Quarkus"),
    (&["micronaut"], "Micronaut"),
    (&["hibernate"], "Hibernate"),
    (&["jooq"], "jOOQ"),
    (&["jakarta"], "Jakarta EE"),
    (&["javalin"], "Javalin"),
    (&["spark"], "Spark Java"),
    (&["play"], "Play Framework"),
    (&["grails"], "Grails"),
    (&["vaadin"], "Vaadin"),
    (&["dropwizard"], "Dropwizard"),
];

// ── PHP ─────────────────────────────────────────────────

const PHP_FRAMEWORKS: &[(&[&str], &str)] = &[
    (&["laravel/framework"], "Laravel"),
    (&["symfony/framework-bundle"], "Symfony"),
    (&["cakephp/cakephp"], "CakePHP"),
    (&["yiisoft/yii2"], "Yii"),
    (&["slim/slim"], "Slim"),
    (&["codeigniter4/framework"], "CodeIgniter"),
    (&["swoole/swoole"], "Swoole"),
    (&["wordpress/wp-core"], "WordPress"),
];

// ── Elixir ──────────────────────────────────────────────

const ELIXIR_FRAMEWORKS: &[(&[&str], &str)] = &[
    (&["phoenix", "phoenix_live_view"], "Phoenix"),
    (&["phoenix_live_view"], "Phoenix LiveView"),
    (&["ecto"], "Ecto"),
    (&["ash"], "Ash"),
    (&["nerves"], "Nerves"),
    (&["absinthe"], "Absinthe (GraphQL)"),
];

/// Scan package.json content for known frameworks.
pub fn scan_npm(content: &str) -> Vec<DetectedFramework> {
    let deps = crate::pkg::parse_deps(content);
    let mut found = Vec::new();

    for (pkgs, name) in NPM_FRAMEWORKS {
        if pkgs
            .iter()
            .any(|p| deps.keys().any(|d| d == p || d.starts_with(p)))
        {
            found.push(DetectedFramework {
                name,
                ecosystem: "npm",
            });
        }
    }

    found
}

/// Scan Cargo.toml content for known frameworks.
pub fn scan_cargo(content: &str) -> Vec<DetectedFramework> {
    let mut found = Vec::new();

    let Ok(val) = content.parse::<toml::Value>() else {
        return found;
    };
    let mut deps: Vec<&str> = Vec::new();
    for key in ["dependencies", "dev-dependencies"] {
        if let Some(table) = val.get(key).and_then(|v| v.as_table()) {
            deps.extend(table.keys().map(String::as_str));
        }
    }

    for (crates, name) in CARGO_FRAMEWORKS {
        if crates.iter().any(|c| deps.contains(c)) {
            found.push(DetectedFramework {
                name,
                ecosystem: "cargo",
            });
        }
    }

    found
}

/// Scan pyproject.toml or requirements.txt for known frameworks.
pub fn scan_python(content: &str) -> Vec<DetectedFramework> {
    let mut found = Vec::new();

    for (pkgs, name) in PYTHON_FRAMEWORKS {
        if pkgs.iter().any(|p| content.contains(p)) {
            found.push(DetectedFramework {
                name,
                ecosystem: "python",
            });
        }
    }

    found
}

/// Scan go.mod for known Go frameworks.
pub fn scan_go(content: &str) -> Vec<DetectedFramework> {
    let mut found = Vec::new();

    for (pkgs, name) in GO_FRAMEWORKS {
        if pkgs.iter().any(|p| content.contains(p)) {
            found.push(DetectedFramework {
                name,
                ecosystem: "go",
            });
        }
    }

    found
}

/// Scan Gemfile or Gemfile.lock for known Ruby frameworks.
pub fn scan_ruby(content: &str) -> Vec<DetectedFramework> {
    let mut found = Vec::new();

    for (pkgs, name) in RUBY_FRAMEWORKS {
        if pkgs.iter().any(|p| content.contains(p)) {
            found.push(DetectedFramework {
                name,
                ecosystem: "ruby",
            });
        }
    }

    found
}

/// Scan build.gradle, pom.xml, or gradle.properties for known Java/JVM frameworks.
pub fn scan_java(content: &str) -> Vec<DetectedFramework> {
    let mut found = Vec::new();

    for (pkgs, name) in JAVA_FRAMEWORKS {
        if pkgs.iter().any(|p| content.contains(p)) {
            found.push(DetectedFramework {
                name,
                ecosystem: "java",
            });
        }
    }

    found
}

/// Scan composer.json for known PHP frameworks.
pub fn scan_php(content: &str) -> Vec<DetectedFramework> {
    let mut found = Vec::new();

    let Ok(val) = content.parse::<serde_json::Value>() else {
        return found;
    };
    let mut deps: Vec<&str> = Vec::new();
    for key in ["require", "require-dev"] {
        if let Some(table) = val.get(key).and_then(|v| v.as_object()) {
            deps.extend(table.keys().map(String::as_str));
        }
    }

    for (pkgs, name) in PHP_FRAMEWORKS {
        if pkgs.iter().any(|p| deps.contains(p)) {
            found.push(DetectedFramework {
                name,
                ecosystem: "php",
            });
        }
    }

    found
}

/// Scan mix.exs for known Elixir frameworks.
pub fn scan_elixir(content: &str) -> Vec<DetectedFramework> {
    let mut found = Vec::new();

    for (pkgs, name) in ELIXIR_FRAMEWORKS {
        if pkgs.iter().any(|p| content.contains(p)) {
            found.push(DetectedFramework {
                name,
                ecosystem: "elixir",
            });
        }
    }

    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_rendered_names_match_the_detector() {
        for name in SERVER_RENDERED_FRAMEWORKS {
            assert!(
                NPM_FRAMEWORKS.iter().any(|(_, n)| n == name),
                "{name} is not a detected framework name"
            );
        }
    }

    #[test]
    fn ssr_variants_need_their_adapter() {
        let names = |json: &str| {
            scan_npm(json)
                .into_iter()
                .map(|f| f.name)
                .collect::<Vec<_>>()
        };
        let static_astro = names(r#"{"dependencies":{"astro":"4"}}"#);
        assert!(!static_astro.contains(&"Astro SSR"));
        let ssr_astro = names(r#"{"dependencies":{"astro":"4","@astrojs/node":"8"}}"#);
        assert!(ssr_astro.contains(&"Astro SSR"));
        let kit =
            names(r#"{"devDependencies":{"@sveltejs/kit":"2","@sveltejs/adapter-static":"3"}}"#);
        assert!(!kit.contains(&"SvelteKit SSR"));
        let kit =
            names(r#"{"devDependencies":{"@sveltejs/kit":"2","@sveltejs/adapter-auto":"3"}}"#);
        assert!(kit.contains(&"SvelteKit SSR"));
    }

    #[test]
    fn scan_php_reads_composer_require_keys() {
        let content = r#"{
            "require": {"php": "^8.2", "laravel/framework": "^11.0"},
            "require-dev": {"phpunit/phpunit": "^11.0", "symfony/framework-bundle": "^7.0"}
        }"#;
        let names: Vec<_> = scan_php(content).into_iter().map(|f| f.name).collect();
        assert!(names.contains(&"Laravel"), "{names:?}");
        assert!(names.contains(&"Symfony"), "{names:?}");
    }

    #[test]
    fn scan_php_ignores_package_substring_matches() {
        let content = r#"{"require": {"my/laravel-helper": "^1.0"}}"#;
        assert!(scan_php(content).is_empty());
    }
}
