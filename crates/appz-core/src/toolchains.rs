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
    // ── Language runtimes ──────────────────────────────────
    PYTHON,
    RUBY_RUNTIME,
    RUST_RUNTIME,
    BUN_RUNTIME,
    NODE_RUNTIME,
    GO_RUNTIME,
    JAVA_RUNTIME,
    // ── Package managers ───────────────────────────────────
    NPM_PM,
    PNPM_PM,
    BUN_PM,
    YARN_PM,
];

#[allow(dead_code)]
pub static EMBEDDED_FRAMEWORKS: &[Framework] = FRAMEWORKS;
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
    cmds!(C!("cargo build"), NONE!(), NONE!())
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
