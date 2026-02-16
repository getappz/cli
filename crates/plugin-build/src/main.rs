//! Plugin build and publish tool for appz WASM plugins.
//!
//! Builds plugins for wasm32-wasi, injects appz_header, signs with Ed25519,
//! and uploads to CDN (S3/R2 compatible).

use clap::{Parser, Subcommand};
use miette::Result;
use plugin_build::{build, inject_header, package, publish, release, sign, Config};

#[derive(Parser)]
#[command(name = "plugin-build")]
#[command(about = "Build and publish appz WASM plugins to CDN", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to plugins config (default: scripts/plugins.toml)
    #[arg(long, global = true)]
    config: Option<std::path::PathBuf>,

    /// Path to output directory for built artifacts
    #[arg(long, global = true, default_value = "dist/plugins")]
    output: Option<std::path::PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile plugins for wasm32-wasi
    Build {
        /// Specific plugin to build (default: all)
        #[arg(short, long)]
        plugin: Option<String>,
        /// Skip wasm-opt optimization (use if Binaryen not installed)
        #[arg(long)]
        no_wasm_opt: bool,
    },

    /// Inject appz_header custom section into WASM
    Inject {
        /// Input WASM file
        #[arg(short, long)]
        input: std::path::PathBuf,
        /// Output WASM file
        #[arg(short, long)]
        output: std::path::PathBuf,
        /// Plugin ID (e.g. check, ssg-migrator)
        #[arg(long)]
        plugin_id: String,
        /// Minimum CLI version (e.g. 0.1.0)
        #[arg(long, default_value = "0.1.0")]
        min_cli_version: String,
    },

    /// Sign WASM with Ed25519
    Sign {
        /// Path to WASM file
        #[arg(short, long)]
        input: std::path::PathBuf,
        /// Path to signing key (default: APPZ_SIGNING_KEY or scripts/signing_key.key)
        #[arg(long)]
        key: Option<std::path::PathBuf>,
    },

    /// Build + inject + sign + checksum (full package step)
    Package {
        /// Specific plugin to package (default: all)
        #[arg(short, long)]
        plugin: Option<String>,
        /// Skip wasm-opt optimization (use if Binaryen not installed)
        #[arg(long)]
        no_wasm_opt: bool,
    },

    /// Upload packaged plugins to CDN
    Publish {
        /// Specific plugin to publish (default: all in dist/plugins)
        #[arg(short, long)]
        plugin: Option<String>,
        /// Skip upload, only update manifest (for local testing)
        #[arg(long)]
        dry_run: bool,
    },

    /// Bump version, package, and publish (full release workflow for a single plugin)
    Release {
        /// Plugin to release (required: check, wp2md, ssg-migrator, site)
        #[arg(short, long)]
        plugin: String,
        /// Bump version before package
        #[arg(long, value_parser = ["patch", "minor", "major"])]
        bump: Option<String>,
        /// Skip upload, only package and update manifest locally
        #[arg(long)]
        dry_run: bool,
        /// Skip wasm-opt optimization
        #[arg(long)]
        no_wasm_opt: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env from current dir or parents (best-effort, ignored if missing)
    let _ = dotenvy::dotenv();

    let cli = Cli::parse();
    let workspace_root = plugin_build::find_workspace_root().unwrap_or_else(|_| {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    });
    let config_path = cli.config.unwrap_or_else(|| workspace_root.join("scripts").join("plugins.toml"));
    let output_dir = cli.output.unwrap_or_else(|| workspace_root.join("dist").join("plugins"));

    let config = Config::load(&config_path).unwrap_or_default();

    match cli.command {
        Commands::Build { plugin, no_wasm_opt } => {
            build(&config, &output_dir, plugin.as_deref(), no_wasm_opt)?;
        }
        Commands::Inject {
            input,
            output,
            plugin_id,
            min_cli_version,
        } => {
            inject_header(&input, &output, &plugin_id, &min_cli_version)?;
            println!("Injected appz_header into {}", output.display());
        }
        Commands::Sign { input, key } => {
            let key_path = key.or_else(|| {
                std::env::var("APPZ_SIGNING_KEY")
                    .ok()
                    .map(|s| std::path::PathBuf::from(s))
            }).or_else(|| {
                Some(
                    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .parent()
                        .unwrap()
                        .parent()
                        .unwrap()
                        .join("scripts")
                        .join("signing_key.key"),
                )
            });
            sign(&input, key_path.as_deref())?;
            println!("Signed {}", input.display());
        }
        Commands::Package { plugin, no_wasm_opt } => {
            package(&config, &output_dir, plugin.as_deref(), no_wasm_opt)?;
        }
        Commands::Publish { plugin, dry_run } => {
            publish(&config, &output_dir, plugin.as_deref(), dry_run).await?
        }
        Commands::Release {
            plugin,
            bump,
            dry_run,
            no_wasm_opt,
        } => {
            release(
                &config,
                &output_dir,
                &plugin,
                bump.as_deref(),
                dry_run,
                no_wasm_opt,
            )
            .await?
        }
    }

    Ok(())
}
