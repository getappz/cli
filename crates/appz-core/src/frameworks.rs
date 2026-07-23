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
    (&["solid-js", "solid-start"], "SolidJS"),
    (&["@nestjs/core"], "NestJS"),
    (&["express"], "Express"),
    (&["fastify"], "Fastify"),
    (&["hono"], "Hono"),
    (&["elysia"], "Elysia"),
    (&["prisma"], "Prisma"),
    (&["drizzle-orm"], "Drizzle"),
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
