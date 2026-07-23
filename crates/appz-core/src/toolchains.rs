/// Unified framework list adopted from Vercel's `frameworks.ts` — all 75+ entries.
/// Each has detectors + commands inline (Vercel's `settings` pattern).
macro_rules! d {
    ($path:expr) => {
        Detector {
            path: $path,
            is_glob: false,
            match_content: None,
            match_package: None,
        }
    };
    ($path:expr, $re:expr) => {
        Detector {
            path: $path,
            is_glob: false,
            match_content: Some($re),
            match_package: None,
        }
    };
}
macro_rules! d_pkg {
    ($pkg:expr) => {
        Detector {
            path: "package.json",
            is_glob: false,
            match_content: None,
            match_package: Some($pkg),
        }
    };
}
macro_rules! d_glob {
    ($path:expr) => {
        Detector {
            path: $path,
            is_glob: true,
            match_content: None,
            match_package: None,
        }
    };
    ($path:expr, $re:expr) => {
        Detector {
            path: $path,
            is_glob: true,
            match_content: Some($re),
            match_package: None,
        }
    };
}
macro_rules! cmds {
    ($b:expr, $i:expr, $d:expr) => {
        LifecycleCommands {
            build: $b,
            install: $i,
            dev: $d,
            test: None,
            lint: None,
            format: None,
        }
    };
    ($b:expr, $i:expr, $d:expr, $t:expr, $l:expr, $f:expr) => {
        LifecycleCommands {
            build: $b,
            install: $i,
            dev: $d,
            test: $t,
            lint: $l,
            format: $f,
        }
    };
}
macro_rules! fw {
    ($name:expr, $slug:expr, $plugin:expr, $ver:expr, $def:expr,
     $every:expr, $some:expr,
     $commands:expr
     $(,)?) => {
        fw_ext!(
            $name,
            $slug,
            $plugin,
            $ver,
            $def,
            $every,
            $some,
            $commands,
            &[],
            DetectionConfidence::Strong,
            None,
            None
        )
    };
}
macro_rules! fw_ext {
    ($name:expr, $slug:expr, $plugin:expr, $ver:expr, $def:expr,
     $every:expr, $some:expr,
     $commands:expr,
     $supersedes:expr, $detection_confidence:expr,
     $output_directory:expr, $env_prefix:expr
     $(,)?) => {
        Framework {
            name: $name,
            slug: $slug,
            mise_plugin: $plugin,
            version_files: $ver,
            default_version: $def,
            detectors: DetectionCriteria {
                every: $every,
                some: $some,
            },
            commands: $commands,
            supersedes: $supersedes,
            detection_confidence: $detection_confidence,
            output_directory: $output_directory,
            env_prefix: $env_prefix,
        }
    };
}
#[derive(Debug, Clone, Copy)]
pub struct Detector {
    pub path: &'static str,
    pub is_glob: bool,
    pub match_content: Option<&'static str>,
    pub match_package: Option<&'static str>,
}
#[derive(Debug, Clone, Copy)]
pub struct DetectionCriteria {
    pub every: &'static [Detector],
    pub some: &'static [Detector],
}
#[derive(Debug, Clone, Copy)]
pub enum DetectionConfidence {
    Strong,
    Weak,
}
#[derive(Debug, Clone, Copy)]
pub struct LifecycleCommands {
    pub build: Option<&'static str>,
    pub install: Option<&'static str>,
    pub dev: Option<&'static str>,
    pub test: Option<&'static str>,
    pub lint: Option<&'static str>,
    pub format: Option<&'static str>,
}
#[derive(Debug, Clone, Copy)]
pub struct Framework {
    pub name: &'static str,
    pub slug: &'static str,
    pub detectors: DetectionCriteria,
    pub mise_plugin: &'static str,
    pub version_files: &'static [&'static str],
    pub default_version: &'static str,
    pub commands: LifecycleCommands,
    pub supersedes: &'static [&'static str],
    pub detection_confidence: DetectionConfidence,
    pub output_directory: Option<&'static str>,
    pub env_prefix: Option<&'static str>,
}
/// Short alias macro: `C!("cmd")` = `Some("cmd")`, `NONE` = `None`
macro_rules! C {
    ($s:expr) => {
        Some($s)
    };
}
macro_rules! NONE {
    () => {
        None
    };
}
pub static FRAMEWORKS: &[Framework] = &[
    // ── JS meta-frameworks ──────────────────────────────────
    CONTAINER,
    BLITZJS_LEGACY,
    NEXTJS,
    GATSBY,
    REMIX,
    REACT_ROUTER,
    ASTRO,
    HEXO,
    ELEVENTY,
    DOCUSAURUS_V2,
    DOCUSAURUS_V1,
    PREACT,
    SOLIDSTART_V1,
    SOLIDSTART_V0,
    DOJO,
    EMBER,
    VUE,
    SCULLY,
    IONIC_ANGULAR,
    ANGULAR,
    POLYMER,
    SVELTE,
    SVELTEKIT_V0,
    SVELTEKIT,
    IONIC_REACT,
    CRA,
    GRIDSOME,
    UMIJS,
    SAPPER,
    SABER,
    STENCIL,
    NUXT,
    REDWOODJS,
    HUGO,
    JEKYLL,
    BRUNCH,
    MIDDLEMAN,
    ZOLA,
    HYDROGEN,
    VITE,
    TANSTACK_START,
    VITEPRESS,
    VUEPRESS,
    PARCEL,
    // ── Python frameworks ──────────────────────────────────
    FASTAPI,
    FLASK,
    FASTHTML,
    DJANGO,
    // ── Other backend frameworks ───────────────────────────
    ASH,
    EVE,
    NITRO,
    HONO,
    EXPRESS,
    H3,
    KOA,
    NESTJS,
    ELYSIA,
    FASTIFY,
    // ── CMS / platforms ────────────────────────────────────
    SANITY,
    SANITY_V2,
    STORYBOOK,
    MCP,
    // ── Language runtimes ──────────────────────────────────
    PYTHON,
    RUBY_RUNTIME,
    RUST_RUNTIME,
    RUST_AXUM,
    RUST_ACTIX,
    BUN_RUNTIME,
    NODE_RUNTIME,
    GO_RUNTIME,
    // ── Go frameworks ─────────────────────────────────────
    GIN,
    ECHO,
    CHI,
    FIBER,
    BUFFALO,
    TEMPL,
    // ── Ruby frameworks ───────────────────────────────────
    RAILS,
    SINATRA,
    HANAMI,
    // ── Java/JVM ──────────────────────────────────────────
    JAVA_RUNTIME,
    SPRING_BOOT,
    QUARKUS,
    MICRONAUT,
    // ── Elixir frameworks ─────────────────────────────────
    PHOENIX,
    // ── Monorepo managers ──────────────────────────────────
    TURBOREPO,
    NX,
    RUSH,
    // ── Package managers ───────────────────────────────────
    NPM_PM,
    PNPM_PM,
    BUN_PM,
    YARN_PM,
    // ── Infrastructure ─────────────────────────────────────
    DOCKER,
    TERRAFORM,
    // ── Databases ──────────────────────────────────────────
    POSTGRES,
    REDIS,
    MASTRA,
];
// ── JS meta-frameworks (mise_plugin: "node") ─────────────
pub static CONTAINER: Framework = fw!(
    "Container",
    "container",
    "docker",
    &[],
    "latest",
    &[],
    &[d!("Dockerfile.vercel"), d!("Containerfile.vercel")],
    cmds!(C!("docker build ."), NONE!(), NONE!()),
);
pub static BLITZJS_LEGACY: Framework = fw_ext!(
    "Blitz.js (Legacy)",
    "blitzjs",
    "node",
    &[],
    "lts",
    &[],
    &[d!("blitz.config.js"), d!("blitz.config.ts")],
    cmds!(C!("blitz build"), C!("npm install"), C!("blitz start")),
    &["hydrogen", "vite", "node"],
    DetectionConfidence::Strong,
    None,
    Some("NEXT_PUBLIC_"),
);
pub static NEXTJS: Framework = fw_ext!(
    "Next.js",
    "nextjs",
    "node",
    &[".nvmrc", ".node-version"],
    "lts",
    &[d_pkg!("next")],
    &[],
    cmds!(
        C!("next build"),
        C!("npm install"),
        C!("next dev --port $PORT")
    ),
    &[],
    DetectionConfidence::Strong,
    Some(".next"),
    Some("NEXT_PUBLIC_"),
);
pub static GATSBY: Framework = fw_ext!(
    "Gatsby.js",
    "gatsby",
    "node",
    &[],
    "lts",
    &[d_pkg!("gatsby")],
    &[],
    cmds!(
        C!("gatsby build"),
        C!("npm install"),
        C!("gatsby develop --port $PORT")
    ),
    &[],
    DetectionConfidence::Strong,
    Some("public"),
    Some("GATSBY_"),
);
pub static REMIX: Framework = fw_ext!(
    "Remix",
    "remix",
    "node",
    &[],
    "lts",
    &[],
    &[
        d_pkg!("@remix-run/dev"),
        d!("remix.config.js"),
        d!("remix.config.mjs")
    ],
    cmds!(C!("remix build"), C!("npm install"), C!("remix dev")),
    &["vite", "node"],
    DetectionConfidence::Strong,
    None,
    None,
);
pub static REACT_ROUTER: Framework = fw_ext!(
    "React Router",
    "react-router",
    "node",
    &[],
    "lts",
    &[],
    &[d!("react-router.config.js"), d!("react-router.config.ts")],
    cmds!(
        C!("react-router build"),
        C!("npm install"),
        C!("react-router dev")
    ),
    &["vite", "node"],
    DetectionConfidence::Strong,
    Some("build"),
    None,
);
pub static ASTRO: Framework = fw_ext!(
    "Astro",
    "astro",
    "node",
    &[],
    "lts",
    &[d_pkg!("astro")],
    &[],
    cmds!(
        C!("astro build"),
        C!("npm install"),
        C!("astro dev --port $PORT")
    ),
    &[],
    DetectionConfidence::Strong,
    Some("dist"),
    Some("PUBLIC_"),
);
pub static HEXO: Framework = fw!(
    "Hexo",
    "hexo",
    "node",
    &[],
    "lts",
    &[d_pkg!("hexo")],
    &[],
    cmds!(
        C!("hexo generate"),
        C!("npm install"),
        C!("hexo server --port $PORT")
    ),
);
pub static ELEVENTY: Framework = fw!(
    "Eleventy",
    "eleventy",
    "node",
    &[],
    "lts",
    &[d_pkg!("@11ty/eleventy")],
    &[],
    cmds!(
        C!("npx @11ty/eleventy"),
        C!("npm install"),
        C!("npx @11ty/eleventy --serve --watch --port $PORT")
    ),
);
pub static DOCUSAURUS_V2: Framework = fw!(
    "Docusaurus (v2+)",
    "docusaurus-2",
    "node",
    &[],
    "lts",
    &[],
    &[d_pkg!("@docusaurus/core")],
    cmds!(
        C!("docusaurus build"),
        C!("npm install"),
        C!("docusaurus start --port $PORT")
    ),
);
pub static DOCUSAURUS_V1: Framework = fw!(
    "Docusaurus (v1)",
    "docusaurus",
    "node",
    &[],
    "lts",
    &[],
    &[d_pkg!("docusaurus")],
    cmds!(
        C!("docusaurus-build"),
        C!("npm install"),
        C!("docusaurus-start --port $PORT")
    ),
);
pub static PREACT: Framework = fw_ext!(
    "Preact",
    "preact",
    "node",
    &[],
    "lts",
    &[d_pkg!("preact-cli")],
    &[],
    cmds!(
        C!("preact build"),
        C!("npm install"),
        C!("preact watch --port $PORT")
    ),
    &[],
    DetectionConfidence::Strong,
    Some("build"),
    None,
);
pub static SOLIDSTART_V1: Framework = fw_ext!(
    "SolidStart (v1)",
    "solidstart-1",
    "node",
    &[],
    "lts",
    &[d_pkg!("solid-js"), d_pkg!("@solidjs/start")],
    &[],
    cmds!(C!("vinxi build"), C!("npm install"), C!("vinxi dev")),
    &["vite"],
    DetectionConfidence::Strong,
    Some(".output"),
    Some("VITE_"),
);
pub static SOLIDSTART_V0: Framework = fw_ext!(
    "SolidStart (v0)",
    "solidstart",
    "node",
    &[],
    "lts",
    &[d_pkg!("solid-js"), d_pkg!("solid-start")],
    &[],
    cmds!(
        C!("solid-start build"),
        C!("npm install"),
        C!("solid-start dev")
    ),
    &["vite"],
    DetectionConfidence::Strong,
    Some(".output"),
    Some("VITE_"),
);
pub static DOJO: Framework = fw!(
    "Dojo",
    "dojo",
    "node",
    &[],
    "lts",
    &[],
    &[d_pkg!("@dojo/framework"), d!(".dojorc")],
    cmds!(
        C!("dojo build"),
        C!("npm install"),
        C!("dojo build -m dev -w -s -p $PORT")
    ),
);
pub static EMBER: Framework = fw!(
    "Ember.js",
    "ember",
    "node",
    &[],
    "lts",
    &[],
    &[d_pkg!("ember-source"), d_pkg!("ember-cli")],
    cmds!(
        C!("ember build"),
        C!("npm install"),
        C!("ember serve --port $PORT")
    ),
);
pub static VUE: Framework = fw_ext!(
    "Vue.js",
    "vue",
    "node",
    &[],
    "lts",
    &[d_pkg!("@vue/cli-service")],
    &[],
    cmds!(
        C!("vue-cli-service build"),
        C!("npm install"),
        C!("vue-cli-service serve --port $PORT")
    ),
    &[],
    DetectionConfidence::Strong,
    Some("dist"),
    Some("VUE_APP_"),
);
pub static SCULLY: Framework = fw!(
    "Scully",
    "scully",
    "node",
    &[],
    "lts",
    &[d_pkg!("@scullyio/init")],
    &[],
    cmds!(
        C!("ng build && scully"),
        C!("npm install"),
        C!("ng serve --port $PORT")
    ),
);
pub static IONIC_ANGULAR: Framework = fw!(
    "Ionic Angular",
    "ionic-angular",
    "node",
    &[],
    "lts",
    &[d_pkg!("@ionic/angular")],
    &[],
    cmds!(
        C!("ng build"),
        C!("npm install"),
        C!("ng serve --port $PORT")
    ),
);
pub static ANGULAR: Framework = fw_ext!(
    "Angular",
    "angular",
    "node",
    &[],
    "lts",
    &[d_pkg!("@angular/cli")],
    &[],
    cmds!(
        C!("ng build"),
        C!("npm install"),
        C!("ng serve --port $PORT")
    ),
    &[],
    DetectionConfidence::Strong,
    Some("dist"),
    None,
);
pub static POLYMER: Framework = fw!(
    "Polymer",
    "polymer",
    "node",
    &[],
    "lts",
    &[d_pkg!("polymer-cli")],
    &[],
    cmds!(
        C!("polymer build"),
        C!("npm install"),
        C!("polymer serve --port $PORT")
    ),
);
pub static SVELTE: Framework = fw_ext!(
    "Svelte",
    "svelte",
    "node",
    &[],
    "lts",
    &[d_pkg!("svelte")],
    &[d_pkg!("sirv-cli")],
    cmds!(C!("rollup -c"), C!("npm install"), C!("rollup -c -w")),
    &[],
    DetectionConfidence::Strong,
    Some("public"),
    None,
);
// NOTE: match_package only checks for a dependency *key* in package.json,
// not its version — so a version-suffixed key like "@sveltejs/kit@1.0.0-next"
// below can never match a real package.json (versions live in the value).
// This detector is effectively dead for pre-1.0 SvelteKit; left as-is rather
// than dropping the version suffix, which would make it match every current
// SvelteKit project alongside SVELTEKIT below (duplicate detection).
pub static SVELTEKIT_V0: Framework = fw_ext!(
    "SvelteKit (v0)",
    "sveltekit",
    "node",
    &[],
    "lts",
    &[],
    &[d_pkg!("@sveltejs/kit@1.0.0-next")],
    cmds!(
        C!("svelte-kit build"),
        C!("npm install"),
        C!("svelte-kit dev --port $PORT")
    ),
    &["vite"],
    DetectionConfidence::Strong,
    Some("public"),
    Some("VITE_"),
);
pub static SVELTEKIT: Framework = fw_ext!(
    "SvelteKit",
    "sveltekit-1",
    "node",
    &[],
    "lts",
    &[d_pkg!("@sveltejs/kit")],
    &[],
    cmds!(
        C!("vite build"),
        C!("npm install"),
        C!("vite dev --port $PORT")
    ),
    &["vite"],
    DetectionConfidence::Strong,
    None,
    None,
);
pub static IONIC_REACT: Framework = fw_ext!(
    "Ionic React",
    "ionic-react",
    "node",
    &[],
    "lts",
    &[d_pkg!("@ionic/react")],
    &[],
    cmds!(
        C!("react-scripts build"),
        C!("npm install"),
        C!("react-scripts start")
    ),
    &[],
    DetectionConfidence::Strong,
    Some("build"),
    None,
);
pub static CRA: Framework = fw_ext!(
    "Create React App",
    "create-react-app",
    "node",
    &[],
    "lts",
    &[],
    &[d_pkg!("react-scripts"), d_pkg!("react-dev-utils")],
    cmds!(
        C!("react-scripts build"),
        C!("npm install"),
        C!("react-scripts start")
    ),
    &[],
    DetectionConfidence::Strong,
    Some("build"),
    Some("REACT_APP_"),
);
pub static GRIDSOME: Framework = fw!(
    "Gridsome",
    "gridsome",
    "node",
    &[],
    "lts",
    &[d_pkg!("gridsome")],
    &[],
    cmds!(
        C!("gridsome build"),
        C!("npm install"),
        C!("gridsome develop -p $PORT")
    ),
);
pub static UMIJS: Framework = fw!(
    "UmiJS",
    "umijs",
    "node",
    &[],
    "lts",
    &[d_pkg!("umi")],
    &[],
    cmds!(
        C!("umi build"),
        C!("npm install"),
        C!("umi dev --port $PORT")
    ),
);
pub static SAPPER: Framework = fw!(
    "Sapper",
    "sapper",
    "node",
    &[],
    "lts",
    &[d_pkg!("sapper")],
    &[],
    cmds!(
        C!("sapper export"),
        C!("npm install"),
        C!("sapper dev --port $PORT")
    ),
);
pub static SABER: Framework = fw!(
    "Saber",
    "saber",
    "node",
    &[],
    "lts",
    &[d_pkg!("saber")],
    &[],
    cmds!(
        C!("saber build"),
        C!("npm install"),
        C!("saber --port $PORT")
    ),
);
pub static STENCIL: Framework = fw!(
    "Stencil",
    "stencil",
    "node",
    &[],
    "lts",
    &[d_pkg!("@stencil/core")],
    &[],
    cmds!(
        C!("stencil build"),
        C!("npm install"),
        C!("stencil build --dev --watch --serve --port $PORT")
    ),
);
pub static NUXT: Framework = fw_ext!(
    "Nuxt",
    "nuxtjs",
    "node",
    &[],
    "lts",
    &[],
    &[
        d_pkg!("nuxt"),
        d_pkg!("nuxt3"),
        d_pkg!("nuxt-edge"),
        d_pkg!("nuxt-nightly")
    ],
    cmds!(C!("nuxt build"), C!("npm install"), C!("nuxt dev")),
    &["nitro"],
    DetectionConfidence::Strong,
    Some("dist"),
    Some("NUXT_ENV_"),
);
pub static REDWOODJS: Framework = fw_ext!(
    "RedwoodJS",
    "redwoodjs",
    "node",
    &[],
    "lts",
    &[d_pkg!("@redwoodjs/core")],
    &[],
    cmds!(
        C!("yarn rw deploy vercel"),
        C!("yarn install"),
        C!("yarn rw dev")
    ),
    &[],
    DetectionConfidence::Strong,
    None,
    Some("REDWOOD_ENV_"),
);
pub static HUGO: Framework = fw!(
    "Hugo",
    "hugo",
    "go",
    &[],
    "latest",
    &[],
    &[
        d!("config.yaml", "baseURL"),
        d!("config.toml", "baseURL"),
        d!("config.json", "baseURL")
    ],
    cmds!(C!("hugo --gc"), NONE!(), C!("hugo server -D -w -p $PORT")),
);
pub static JEKYLL: Framework = fw!(
    "Jekyll",
    "jekyll",
    "ruby",
    &[],
    "latest",
    &[d!("_config.yml")],
    &[],
    cmds!(
        C!("jekyll build"),
        C!("bundle install"),
        C!("bundle exec jekyll serve --watch --port $PORT")
    ),
);
pub static BRUNCH: Framework = fw!(
    "Brunch",
    "brunch",
    "node",
    &[],
    "lts",
    &[],
    &[d_pkg!("brunch"), d!("brunch-config.js")],
    cmds!(
        C!("brunch build --production"),
        C!("npm install"),
        C!("brunch watch --server --port $PORT")
    ),
);
pub static MIDDLEMAN: Framework = fw!(
    "Middleman",
    "middleman",
    "ruby",
    &[],
    "latest",
    &[d!("config.rb")],
    &[],
    cmds!(
        C!("bundle exec middleman build"),
        C!("bundle install"),
        C!("bundle exec middleman server -p $PORT")
    ),
);
pub static ZOLA: Framework = fw!(
    "Zola",
    "zola",
    "zola",
    &[],
    "latest",
    &[d!("config.toml")],
    &[],
    cmds!(C!("zola build"), NONE!(), C!("zola serve --port $PORT")),
);
pub static HYDROGEN: Framework = fw_ext!(
    "Hydrogen (v1)",
    "hydrogen",
    "node",
    &[],
    "lts",
    &[d_pkg!("@shopify/hydrogen")],
    &[],
    cmds!(C!("npm run build"), C!("npm install"), C!("npm run dev")),
    &["vite"],
    DetectionConfidence::Strong,
    Some("dist"),
    None,
);
pub static VITE: Framework = fw_ext!(
    "Vite",
    "vite",
    "node",
    &[],
    "lts",
    &[d_pkg!("vite")],
    &[],
    cmds!(
        C!("vite build"),
        C!("npm install"),
        C!("vite dev --port $PORT")
    ),
    &["ionic-react"],
    DetectionConfidence::Strong,
    Some("dist"),
    Some("VITE_"),
);
pub static TANSTACK_START: Framework = fw!(
    "TanStack Start",
    "tanstack-start",
    "node",
    &[],
    "lts",
    &[d_pkg!("@tanstack/start")],
    &[],
    cmds!(C!("npm run build"), C!("npm install"), C!("npm run dev")),
);
pub static VITEPRESS: Framework = fw!(
    "VitePress",
    "vitepress",
    "node",
    &[],
    "lts",
    &[d_pkg!("vitepress")],
    &[],
    cmds!(
        C!("vitepress build"),
        C!("npm install"),
        C!("vitepress dev --port $PORT")
    ),
);
pub static VUEPRESS: Framework = fw!(
    "VuePress",
    "vuepress",
    "node",
    &[],
    "lts",
    &[d_pkg!("vuepress")],
    &[],
    cmds!(
        C!("vuepress build"),
        C!("npm install"),
        C!("vuepress dev --port $PORT")
    ),
);
pub static PARCEL: Framework = fw!(
    "Parcel",
    "parcel",
    "node",
    &[],
    "lts",
    &[d_pkg!("parcel")],
    &[],
    cmds!(
        C!("parcel build"),
        C!("npm install"),
        C!("parcel serve --port $PORT")
    ),
);
// ── Python frameworks (mise_plugin: "python") ───────────
pub static FASTAPI: Framework = fw!(
    "FastAPI",
    "fastapi",
    "python",
    &[],
    "latest",
    &[d_pkg!("fastapi")],
    &[],
    cmds!(
        NONE!(),
        C!("pip install -r requirements.txt"),
        C!("uvicorn main:app")
    ),
);
pub static FLASK: Framework = fw!(
    "Flask",
    "flask",
    "python",
    &[],
    "latest",
    &[d_pkg!("flask")],
    &[],
    cmds!(
        NONE!(),
        C!("pip install -r requirements.txt"),
        C!("flask run")
    ),
);
pub static FASTHTML: Framework = fw!(
    "FastHTML",
    "fasthtml",
    "python",
    &[],
    "latest",
    &[d_pkg!("fasthtml")],
    &[],
    cmds!(
        NONE!(),
        C!("pip install -r requirements.txt"),
        C!("python main.py")
    ),
);
pub static DJANGO: Framework = fw!(
    "Django",
    "django",
    "python",
    &[],
    "latest",
    &[d_pkg!("django")],
    &[],
    cmds!(
        C!("python manage.py collectstatic"),
        C!("pip install -r requirements.txt"),
        C!("python manage.py runserver")
    ),
);
// ── Other backend frameworks (mise_plugin: "node") ──────
pub static ASH: Framework = fw!(
    "Ash",
    "ash",
    "elixir",
    &[],
    "latest",
    &[d!("mix.exs", "ash")],
    &[],
    cmds!(C!("mix compile"), C!("mix deps.get"), C!("mix phx.server")),
);
pub static EVE: Framework = fw!(
    "eve",
    "eve",
    "python",
    &[],
    "latest",
    &[d_pkg!("eve")],
    &[],
    cmds!(NONE!(), C!("pip install -r requirements.txt"), NONE!())
);
pub static NITRO: Framework = fw_ext!(
    "Nitro",
    "nitro",
    "node",
    &[],
    "lts",
    &[d_pkg!("nitropack"), d_pkg!("nitro")],
    &[],
    cmds!(C!("nitro build"), C!("npm install"), C!("nitro dev")),
    &["vite"],
    DetectionConfidence::Strong,
    Some("dist"),
    None,
);
pub static HONO: Framework = fw_ext!(
    "Hono",
    "hono",
    "node",
    &[],
    "lts",
    &[d_pkg!("hono")],
    &[],
    cmds!(NONE!(), C!("npm install"), NONE!()),
    &["node"],
    DetectionConfidence::Strong,
    None,
    None,
);
pub static EXPRESS: Framework = fw_ext!(
    "Express",
    "express",
    "node",
    &[],
    "lts",
    &[d_pkg!("express")],
    &[],
    cmds!(NONE!(), C!("npm install"), NONE!()),
    &["node"],
    DetectionConfidence::Strong,
    None,
    None,
);
pub static H3: Framework = fw_ext!(
    "H3",
    "h3",
    "node",
    &[],
    "lts",
    &[d_pkg!("h3")],
    &[],
    cmds!(NONE!(), C!("npm install"), NONE!()),
    &["node"],
    DetectionConfidence::Strong,
    None,
    None,
);
pub static KOA: Framework = fw_ext!(
    "Koa",
    "koa",
    "node",
    &[],
    "lts",
    &[d_pkg!("koa")],
    &[],
    cmds!(NONE!(), C!("npm install"), NONE!()),
    &["node"],
    DetectionConfidence::Strong,
    None,
    None,
);
pub static NESTJS: Framework = fw_ext!(
    "NestJS",
    "nestjs",
    "node",
    &[],
    "lts",
    &[d_pkg!("@nestjs/core")],
    &[],
    cmds!(C!("nest build"), C!("npm install"), C!("nest start")),
    &["node"],
    DetectionConfidence::Strong,
    None,
    None,
);
pub static ELYSIA: Framework = fw_ext!(
    "Elysia",
    "elysia",
    "node",
    &[],
    "lts",
    &[d_pkg!("elysia")],
    &[],
    cmds!(NONE!(), C!("npm install"), NONE!()),
    &["node"],
    DetectionConfidence::Strong,
    None,
    None,
);
pub static FASTIFY: Framework = fw_ext!(
    "Fastify",
    "fastify",
    "node",
    &[],
    "lts",
    &[d_pkg!("fastify")],
    &[],
    cmds!(NONE!(), C!("npm install"), NONE!()),
    &["node"],
    DetectionConfidence::Strong,
    None,
    None,
);
// ── CMS / platforms ─────────────────────────────────────
pub static SANITY: Framework = fw_ext!(
    "Sanity",
    "sanity",
    "node",
    &[],
    "lts",
    &[d_pkg!("sanity")],
    &[],
    cmds!(C!("npm run build"), C!("npm install"), C!("npm run dev")),
    &[],
    DetectionConfidence::Strong,
    None,
    Some("SANITY_STUDIO_"),
);
pub static SANITY_V2: Framework = fw_ext!(
    "Sanity (v2 - legacy)",
    "sanity-v2-legacy",
    "node",
    &[],
    "lts",
    &[d_pkg!("@sanity/core")],
    &[],
    cmds!(
        C!("sanity build"),
        C!("npm install"),
        C!("sanity start --port $PORT")
    ),
    &[],
    DetectionConfidence::Strong,
    None,
    Some("SANITY_STUDIO_"),
);
pub static STORYBOOK: Framework = fw_ext!(
    "Storybook",
    "storybook",
    "node",
    &[],
    "lts",
    &[d_pkg!("@storybook/react")],
    &[],
    cmds!(
        C!("build-storybook"),
        C!("npm install"),
        C!("storybook dev --port $PORT")
    ),
    &[],
    DetectionConfidence::Weak,
    None,
    None,
);
pub static MCP: Framework = fw!(
    "xmcp",
    "xmcp",
    "node",
    &[],
    "lts",
    &[],
    &[d!("mcp.json")],
    cmds!(NONE!(), C!("npm install"), NONE!())
);
// ── Language runtimes ───────────────────────────────────
pub static PYTHON: Framework = fw!(
    "Python",
    "python",
    "python",
    &[".python-version"],
    "latest",
    &[],
    &[
        d!("requirements.txt"),
        d!("pyproject.toml"),
        d!("setup.py"),
        d!("Pipfile")
    ],
    cmds!(NONE!(), NONE!(), NONE!())
);
pub static RUBY_RUNTIME: Framework = fw!(
    "Ruby",
    "ruby",
    "ruby",
    &[".ruby-version"],
    "latest",
    &[],
    &[d!("Gemfile"), d!("Gemfile.lock")],
    cmds!(NONE!(), NONE!(), NONE!())
);
pub static RUST_RUNTIME: Framework = fw!(
    "Rust",
    "rust",
    "rust",
    &["rust-toolchain.toml", "rust-toolchain"],
    "latest",
    &[],
    &[d!("Cargo.toml")],
    cmds!(NONE!(), NONE!(), NONE!())
);
pub static RUST_AXUM: Framework = fw_ext!(
    "Axum",
    "axum",
    "rust",
    &[],
    "latest",
    &[d_pkg!("axum")],
    &[],
    cmds!(C!("cargo build"), NONE!(), C!("cargo run")),
    &["rust"],
    DetectionConfidence::Strong,
    None,
    None,
);
pub static RUST_ACTIX: Framework = fw_ext!(
    "Actix Web",
    "actix-web",
    "rust",
    &[],
    "latest",
    &[d_pkg!("actix-web")],
    &[],
    cmds!(C!("cargo build"), NONE!(), C!("cargo run")),
    &["rust"],
    DetectionConfidence::Strong,
    None,
    None,
);
pub static BUN_RUNTIME: Framework = fw!(
    "Bun",
    "bun",
    "bun",
    &[],
    "latest",
    &[],
    &[d!("bun.lockb"), d!("bun.lock")],
    cmds!(NONE!(), NONE!(), NONE!())
);
pub static NODE_RUNTIME: Framework = fw!(
    "Node",
    "node",
    "node",
    &[".nvmrc", ".node-version"],
    "lts",
    &[],
    &[d!("package.json"), d!(".nvmrc"), d!(".node-version")],
    cmds!(NONE!(), NONE!(), NONE!())
);
pub static GO_RUNTIME: Framework = fw!(
    "Go",
    "go",
    "go",
    &[".go-version"],
    "latest",
    &[],
    &[d!("go.mod")],
    cmds!(NONE!(), NONE!(), NONE!())
);
// ── Go frameworks (mise_plugin: "go") ──────────────────
pub static GIN: Framework = fw_ext!(
    "Gin",
    "gin",
    "go",
    &[],
    "latest",
    &[d!("go.mod", "gin-gonic/gin")],
    &[],
    cmds!(C!("go build -o ./bin/main ."), NONE!(), C!("go run .")),
    &["go"],
    DetectionConfidence::Strong,
    None,
    None,
);
pub static ECHO: Framework = fw_ext!(
    "Echo",
    "echo",
    "go",
    &[],
    "latest",
    &[d!("go.mod", "labstack/echo")],
    &[],
    cmds!(C!("go build -o ./bin/main ."), NONE!(), C!("go run .")),
    &["go"],
    DetectionConfidence::Strong,
    None,
    None,
);
pub static CHI: Framework = fw_ext!(
    "Chi",
    "chi",
    "go",
    &[],
    "latest",
    &[d!("go.mod", "go-chi/chi")],
    &[],
    cmds!(C!("go build -o ./bin/main ."), NONE!(), C!("go run .")),
    &["go"],
    DetectionConfidence::Strong,
    None,
    None,
);
pub static FIBER: Framework = fw_ext!(
    "Fiber",
    "fiber",
    "go",
    &[],
    "latest",
    &[d!("go.mod", "gofiber/fiber")],
    &[],
    cmds!(C!("go build -o ./bin/main ."), NONE!(), C!("go run .")),
    &["go"],
    DetectionConfidence::Strong,
    None,
    None,
);
pub static BUFFALO: Framework = fw_ext!(
    "Buffalo",
    "buffalo",
    "go",
    &[],
    "latest",
    &[d!("go.mod", "gobuffalo/buffalo")],
    &[],
    cmds!(C!("buffalo build"), NONE!(), C!("buffalo dev")),
    &["go"],
    DetectionConfidence::Strong,
    None,
    None,
);
pub static TEMPL: Framework = fw_ext!(
    "Templ",
    "templ",
    "go",
    &[],
    "latest",
    &[d!("go.mod", "a-h/templ")],
    &[],
    cmds!(
        C!("go build -o ./bin/main ."),
        NONE!(),
        C!("templ generate && go run .")
    ),
    &["go"],
    DetectionConfidence::Strong,
    None,
    None,
);
// ── Ruby frameworks (mise_plugin: "ruby") ───────────────
pub static RAILS: Framework = fw_ext!(
    "Ruby on Rails",
    "rails",
    "ruby",
    &[],
    "latest",
    &[d!("Gemfile", "rails")],
    &[],
    cmds!(
        C!("rails assets:precompile"),
        C!("bundle install"),
        C!("rails server -p $PORT")
    ),
    &["ruby"],
    DetectionConfidence::Strong,
    Some("public"),
    None,
);
pub static SINATRA: Framework = fw_ext!(
    "Sinatra",
    "sinatra",
    "ruby",
    &[],
    "latest",
    &[d!("Gemfile", "sinatra")],
    &[],
    cmds!(NONE!(), C!("bundle install"), C!("ruby app.rb")),
    &["ruby"],
    DetectionConfidence::Strong,
    None,
    None,
);
pub static HANAMI: Framework = fw_ext!(
    "Hanami",
    "hanami",
    "ruby",
    &[],
    "latest",
    &[d!("Gemfile", "hanami")],
    &[],
    cmds!(
        C!("bundle exec hanami assets precompile"),
        C!("bundle install"),
        C!("bundle exec hanami server")
    ),
    &["ruby"],
    DetectionConfidence::Strong,
    Some("public"),
    None,
);
// ── Java/JVM frameworks (mise_plugin: "java") ───────────
pub static JAVA_RUNTIME: Framework = fw!(
    "Java",
    "java",
    "java",
    &[".java-version"],
    "latest",
    &[],
    &[
        d!("pom.xml"),
        d!("build.gradle"),
        d!("build.gradle.kts"),
        d!("settings.gradle")
    ],
    cmds!(NONE!(), NONE!(), NONE!())
);
pub static SPRING_BOOT: Framework = fw_ext!(
    "Spring Boot",
    "spring-boot",
    "java",
    &[],
    "latest",
    &[
        d!("pom.xml", "spring-boot"),
        d!("build.gradle", "spring-boot")
    ],
    &[],
    cmds!(
        C!("mvn package"),
        C!("mvn install"),
        C!("mvn spring-boot:run")
    ),
    &["java"],
    DetectionConfidence::Strong,
    Some("target"),
    None,
);
pub static QUARKUS: Framework = fw_ext!(
    "Quarkus",
    "quarkus",
    "java",
    &[],
    "latest",
    &[d!("pom.xml", "quarkus"), d!("build.gradle", "quarkus")],
    &[],
    cmds!(C!("mvn package"), C!("mvn install"), C!("mvn quarkus:dev")),
    &["java"],
    DetectionConfidence::Strong,
    Some("target"),
    None,
);
pub static MICRONAUT: Framework = fw_ext!(
    "Micronaut",
    "micronaut",
    "java",
    &[],
    "latest",
    &[d!("pom.xml", "micronaut"), d!("build.gradle", "micronaut")],
    &[],
    cmds!(C!("mvn package"), C!("mvn install"), C!("mvn mn:run")),
    &["java"],
    DetectionConfidence::Strong,
    Some("target"),
    None,
);
// ── Elixir frameworks (mise_plugin: "elixir") ───────────
pub static PHOENIX: Framework = fw_ext!(
    "Phoenix",
    "phoenix",
    "elixir",
    &[],
    "latest",
    &[d!("mix.exs", "phoenix")],
    &[],
    cmds!(C!("mix release"), C!("mix deps.get"), C!("mix phx.server")),
    &[],
    DetectionConfidence::Strong,
    None,
    None,
);
// ── Monorepo managers ───────────────────────────────────
pub static TURBOREPO: Framework = fw!(
    "Turborepo",
    "turbo",
    "turbo",
    &[],
    "latest",
    &[],
    &[d!("turbo.json"), d!("turbo.jsonc")],
    cmds!(
        C!("turbo run build"),
        C!("npm install"),
        C!("turbo run dev")
    ),
);
pub static NX: Framework = fw!(
    "Nx",
    "nx",
    "nx",
    &[],
    "latest",
    &[d!("nx.json")],
    &[],
    cmds!(C!("npx nx build"), C!("npm install"), C!("npx nx serve")),
);
pub static RUSH: Framework = fw!(
    "Rush",
    "rush",
    "rush",
    &[],
    "latest",
    &[d!("rush.json")],
    &[],
    cmds!(C!("rush build"), C!("rush install"), C!("rush start")),
);
// ── Package managers ────────────────────────────────────
pub static NPM_PM: Framework = fw!(
    "npm",
    "npm",
    "npm",
    &[],
    "latest",
    &[],
    &[d!("package-lock.json")],
    cmds!(NONE!(), NONE!(), NONE!())
);
pub static PNPM_PM: Framework = fw!(
    "pnpm",
    "pnpm",
    "pnpm",
    &[],
    "latest",
    &[],
    &[d!("pnpm-lock.yaml")],
    cmds!(NONE!(), NONE!(), NONE!())
);
pub static BUN_PM: Framework = fw!(
    "bun",
    "bun",
    "bun",
    &[],
    "latest",
    &[],
    &[d!("bun.lockb"), d!("bun.lock")],
    cmds!(NONE!(), NONE!(), NONE!())
);
pub static YARN_PM: Framework = fw!(
    "yarn",
    "yarn",
    "yarn",
    &[],
    "latest",
    &[],
    &[d!("yarn.lock")],
    cmds!(NONE!(), NONE!(), NONE!())
);
// ── Infrastructure ──────────────────────────────────────
pub static DOCKER: Framework = fw!(
    "Docker",
    "docker",
    "docker",
    &[],
    "latest",
    &[],
    &[d!("Dockerfile"), d!("docker-compose.yml")],
    cmds!(C!("docker build ."), NONE!(), NONE!())
);
pub static TERRAFORM: Framework = fw!(
    "Terraform",
    "terraform",
    "terraform",
    &[".terraform-version"],
    "latest",
    &[],
    &[d_glob!("*.tf")],
    cmds!(C!("terraform apply"), NONE!(), NONE!())
);
// ── Databases ───────────────────────────────────────────
pub static POSTGRES: Framework = fw!(
    "PostgreSQL",
    "postgres",
    "postgres",
    &[],
    "latest",
    &[],
    &[d!("pg_hba.conf"), d!("postgresql.conf")],
    cmds!(NONE!(), NONE!(), NONE!())
);
pub static REDIS: Framework = fw!(
    "Redis",
    "redis",
    "redis",
    &[],
    "latest",
    &[],
    &[d!("redis.conf"), d!("sentinel.conf")],
    cmds!(NONE!(), NONE!(), NONE!())
);
// ── Other ───────────────────────────────────────────────
pub static MASTRA: Framework = fw_ext!(
    "Mastra",
    "mastra",
    "node",
    &[],
    "lts",
    &[d_pkg!("mastra")],
    &[],
    cmds!(C!("npm run build"), C!("npm install"), C!("npm run dev")),
    &["node"],
    DetectionConfidence::Strong,
    None,
    None,
);
