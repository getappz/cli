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

/// Parse a JSON value as an object or return empty.
fn parse_json_deps(content: &str) -> Vec<String> {
    let Ok(val) = self::json::parse(content) else {
        return Vec::new();
    };

    fn get_deps(val: &self::json::Value, key: &str) -> Vec<String> {
        match val {
            self::json::Value::Object(map) => {
                if let Some(self::json::Value::Object(deps)) = map.get(key) {
                    deps.iter().map(|(k, _)| k.clone()).collect()
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }

    let mut deps = get_deps(&val, "dependencies");
    deps.extend(get_deps(&val, "devDependencies"));
    deps
}

/// Scan package.json content for known frameworks.
pub fn scan_npm(content: &str) -> Vec<DetectedFramework> {
    let deps = parse_json_deps(content);
    let mut found = Vec::new();

    for (pkgs, name) in NPM_FRAMEWORKS {
        if pkgs.iter().any(|p| deps.iter().any(|d| d == p || d.starts_with(p))) {
            found.push(DetectedFramework { name, ecosystem: "npm" });
        }
    }

    found
}

/// Scan Cargo.toml content for known frameworks.
pub fn scan_cargo(content: &str) -> Vec<DetectedFramework> {
    let mut found = Vec::new();

    // Simple parser: find [dependencies] or [dev-dependencies] sections,
    // then look for crate names on subsequent lines.
    let in_deps = content
        .lines()
        .skip_while(|l| !l.trim().starts_with('['));
    let deps: Vec<&str> = in_deps
        .take_while(|l| l.trim().starts_with('[') || l.contains('='))
        .filter(|l| l.contains('='))
        .filter_map(|l| l.split('=').next())
        .map(|s| s.trim())
        .collect();

    for (crates, name) in CARGO_FRAMEWORKS {
        if crates.iter().any(|c| deps.contains(c)) {
            found.push(DetectedFramework { name, ecosystem: "cargo" });
        }
    }

    found
}

/// Scan pyproject.toml or requirements.txt for known frameworks.
pub fn scan_python(content: &str) -> Vec<DetectedFramework> {
    let mut found = Vec::new();

    for (pkgs, name) in PYTHON_FRAMEWORKS {
        if pkgs.iter().any(|p| content.contains(p)) {
            found.push(DetectedFramework { name, ecosystem: "python" });
        }
    }

    found
}

/// Scan go.mod for known Go frameworks.
pub fn scan_go(content: &str) -> Vec<DetectedFramework> {
    let mut found = Vec::new();

    for (pkgs, name) in GO_FRAMEWORKS {
        if pkgs.iter().any(|p| content.contains(p)) {
            found.push(DetectedFramework { name, ecosystem: "go" });
        }
    }

    found
}

/// Scan Gemfile or Gemfile.lock for known Ruby frameworks.
pub fn scan_ruby(content: &str) -> Vec<DetectedFramework> {
    let mut found = Vec::new();

    for (pkgs, name) in RUBY_FRAMEWORKS {
        if pkgs.iter().any(|p| content.contains(p)) {
            found.push(DetectedFramework { name, ecosystem: "ruby" });
        }
    }

    found
}

/// Scan build.gradle, pom.xml, or gradle.properties for known Java/JVM frameworks.
pub fn scan_java(content: &str) -> Vec<DetectedFramework> {
    let mut found = Vec::new();

    for (pkgs, name) in JAVA_FRAMEWORKS {
        if pkgs.iter().any(|p| content.contains(p)) {
            found.push(DetectedFramework { name, ecosystem: "java" });
        }
    }

    found
}

/// Scan mix.exs for known Elixir frameworks.
pub fn scan_elixir(content: &str) -> Vec<DetectedFramework> {
    let mut found = Vec::new();

    for (pkgs, name) in ELIXIR_FRAMEWORKS {
        if pkgs.iter().any(|p| content.contains(p)) {
            found.push(DetectedFramework { name, ecosystem: "elixir" });
        }
    }

    found
}

/// Minimal JSON parser for dependency scanning (no serde dep).
#[allow(dead_code)]
mod json {
    use std::collections::BTreeMap;

    #[derive(Debug)]
    pub enum Value {
        Object(BTreeMap<String, Value>),
        Array(Vec<Value>),
        String(String),
        Number(f64),
        Bool(bool),
        Null,
    }

    impl Value {
        pub fn entries(&self) -> Vec<(String, &Value)> {
            match self {
                Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), v)).collect(),
                _ => Vec::new(),
            }
        }

        pub fn as_str(&self) -> Option<&str> {
            match self {
                Value::String(s) => Some(s),
                _ => None,
            }
        }

        pub fn get(&self, key: &str) -> Option<&Value> {
            match self {
                Value::Object(map) => map.get(key),
                _ => None,
            }
        }
    }

    pub fn parse(input: &str) -> Result<Value, String> {
        let chars: Vec<char> = input.chars().collect();
        let mut pos = 0;
        skip_ws(&chars, &mut pos);
        parse_value(&chars, &mut pos)
    }

    fn skip_ws(chars: &[char], pos: &mut usize) {
        while *pos < chars.len() && chars[*pos].is_ascii_whitespace() {
            *pos += 1;
        }
    }

    fn parse_value(chars: &[char], pos: &mut usize) -> Result<Value, String> {
        skip_ws(chars, pos);
        if *pos >= chars.len() {
            return Err("unexpected end".to_string());
        }
        match chars[*pos] {
            '"' => parse_string(chars, pos).map(Value::String),
            '{' => parse_object(chars, pos).map(Value::Object),
            '[' => parse_array(chars, pos).map(Value::Array),
            't' | 'f' => parse_bool(chars, pos).map(Value::Bool),
            'n' => parse_null(chars, pos).map(|_| Value::Null),
            _ if chars[*pos].is_ascii_digit() || chars[*pos] == '-' => {
                parse_number(chars, pos).map(Value::Number)
            }
            c => Err(format!("unexpected char: {c} at pos {pos}")),
        }
    }

    fn parse_string(chars: &[char], pos: &mut usize) -> Result<String, String> {
        assert_eq!(chars[*pos], '"');
        *pos += 1;
        let mut s = String::new();
        while *pos < chars.len() && chars[*pos] != '"' {
            if chars[*pos] == '\\' {
                *pos += 1;
                if *pos < chars.len() {
                    s.push(chars[*pos]);
                    *pos += 1;
                }
            } else {
                s.push(chars[*pos]);
                *pos += 1;
            }
        }
        if *pos < chars.len() {
            *pos += 1; // skip closing "
        }
        Ok(s)
    }

    fn parse_number(chars: &[char], pos: &mut usize) -> Result<f64, String> {
        let start = *pos;
        if chars[*pos] == '-' {
            *pos += 1;
        }
        while *pos < chars.len() && (chars[*pos].is_ascii_digit() || chars[*pos] == '.') {
            *pos += 1;
        }
        let s: String = chars[start..*pos].iter().collect();
        s.parse::<f64>().map_err(|e| format!("{e}"))
    }

    fn parse_bool(chars: &[char], pos: &mut usize) -> Result<bool, String> {
        if chars[*pos..].starts_with(&['t', 'r', 'u', 'e']) {
            *pos += 4;
            Ok(true)
        } else if chars[*pos..].starts_with(&['f', 'a', 'l', 's', 'e']) {
            *pos += 5;
            Ok(false)
        } else {
            Err("expected bool".to_string())
        }
    }

    fn parse_null(chars: &[char], pos: &mut usize) -> Result<(), String> {
        if chars[*pos..].starts_with(&['n', 'u', 'l', 'l']) {
            *pos += 4;
            Ok(())
        } else {
            Err("expected null".to_string())
        }
    }

    fn parse_object(chars: &[char], pos: &mut usize) -> Result<BTreeMap<String, Value>, String> {
        assert_eq!(chars[*pos], '{');
        *pos += 1;
        let mut map = BTreeMap::new();
        loop {
            skip_ws(chars, pos);
            if *pos < chars.len() && chars[*pos] == '}' {
                *pos += 1;
                return Ok(map);
            }
            let key = parse_value(chars, pos)?;
            let key = match key {
                Value::String(s) => s,
                _ => return Err("expected string key".to_string()),
            };
            skip_ws(chars, pos);
            if *pos < chars.len() && chars[*pos] == ':' {
                *pos += 1;
            }
            let val = parse_value(chars, pos)?;
            map.insert(key, val);
            skip_ws(chars, pos);
            if *pos < chars.len() && chars[*pos] == ',' {
                *pos += 1;
            }
        }
    }

    fn parse_array(chars: &[char], pos: &mut usize) -> Result<Vec<Value>, String> {
        assert_eq!(chars[*pos], '[');
        *pos += 1;
        let mut arr = Vec::new();
        loop {
            skip_ws(chars, pos);
            if *pos < chars.len() && chars[*pos] == ']' {
                *pos += 1;
                return Ok(arr);
            }
            arr.push(parse_value(chars, pos)?);
            skip_ws(chars, pos);
            if *pos < chars.len() && chars[*pos] == ',' {
                *pos += 1;
            }
        }
    }
}
