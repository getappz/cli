//! Crawl WASM Plugin
//!
//! Scrape single URLs or crawl entire sites to Markdown. Uses crawl-core for
//! link filtering and sitemap parsing; host functions for fetch and filesystem.

mod crawl;
mod scrape;
mod vfs_wasm;

use appz_pdk::prelude::*;
use appz_pdk::security;
use extism_pdk::*;
use std::collections::HashMap;
use vfs_wasm::WasmVfs;

use crawl::{CrawlOptions, SitemapMode};

// Host functions
#[host_fn]
extern "ExtismHost" {
    fn appz_pfs_read_file(input: Json<PluginFsReadInput>) -> Json<PluginFsReadOutput>;
    fn appz_pfs_write_file(input: Json<PluginFsWriteInput>) -> Json<PluginFsWriteOutput>;
    fn appz_pfs_exists(input: Json<PluginFsReadInput>) -> Json<PluginFsExistsOutput>;
    fn appz_pfs_mkdir(input: Json<PluginFsReadInput>) -> Json<PluginFsWriteOutput>;
    fn appz_phttp_download(
        input: Json<PluginHttpDownloadInput>,
    ) -> Json<PluginHttpDownloadOutput>;
    fn appz_util_info(message: String) -> Json<appz_pdk::VoidResponse>;
}

#[plugin_fn]
pub fn appz_plugin_handshake(
    input: Json<PluginHandshakeChallenge>,
) -> FnResult<Json<PluginHandshakeResponse>> {
    let response = security::compute_handshake(&input.0);
    Ok(Json(response))
}

#[plugin_fn]
pub fn appz_plugin_info() -> FnResult<Json<PluginInfo>> {
    Ok(Json(PluginInfo {
        name: "crawl".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        commands: vec![PluginCommandDef {
            name: "crawl".to_string(),
            about: "Scrape or crawl URLs to Markdown files".to_string(),
            args: crawl_args(),
            subcommands: vec![
                PluginCommandDef {
                    name: "scrape".to_string(),
                    about: "Scrape a single URL to markdown".to_string(),
                    args: scrape_args(),
                    subcommands: vec![],
                },
                PluginCommandDef {
                    name: "crawl".to_string(),
                    about: "Crawl multiple pages from a starting URL".to_string(),
                    args: crawl_sub_args(),
                    subcommands: vec![],
                },
            ],
        }],
    }))
}

fn crawl_args() -> Vec<PluginArgDef> {
    vec![
        PluginArgDef {
            name: "_positional".to_string(),
            short: None,
            long: None,
            help: Some("URL to scrape/crawl, or subcommand (scrape, crawl)".to_string()),
            required: false,
            default: None,
        },
        PluginArgDef {
            name: "url".to_string(),
            short: None,
            long: Some("url".to_string()),
            help: Some("URL to scrape or crawl".to_string()),
            required: false,
            default: None,
        },
        arg_output(),
        PluginArgDef {
            name: "limit".to_string(),
            short: None,
            long: Some("limit".to_string()),
            help: Some("Max URLs (crawl mode)".to_string()),
            required: false,
            default: Some("100".to_string()),
        },
        PluginArgDef {
            name: "max-depth".to_string(),
            short: None,
            long: Some("max-depth".to_string()),
            help: Some("Max crawl depth".to_string()),
            required: false,
            default: Some("10".to_string()),
        },
        PluginArgDef {
            name: "include-paths".to_string(),
            short: None,
            long: Some("include-paths".to_string()),
            help: Some("Include path regexes (comma-separated)".to_string()),
            required: false,
            default: None,
        },
        PluginArgDef {
            name: "exclude-paths".to_string(),
            short: None,
            long: Some("exclude-paths".to_string()),
            help: Some("Exclude path regexes (comma-separated)".to_string()),
            required: false,
            default: None,
        },
        PluginArgDef {
            name: "allow-subdomains".to_string(),
            short: None,
            long: Some("allow-subdomains".to_string()),
            help: Some("Allow subdomains (true/false)".to_string()),
            required: false,
            default: Some("false".to_string()),
        },
        PluginArgDef {
            name: "sitemap".to_string(),
            short: None,
            long: Some("sitemap".to_string()),
            help: Some("Sitemap: skip, include, only".to_string()),
            required: false,
            default: Some("skip".to_string()),
        },
        arg_formats(),
        arg_timeout(),
        arg_strict_ssl(),
    ]
}

fn scrape_args() -> Vec<PluginArgDef> {
    vec![
        PluginArgDef {
            name: "_positional".to_string(),
            short: None,
            long: None,
            help: Some("URL to scrape".to_string()),
            required: false,
            default: None,
        },
        PluginArgDef {
            name: "url".to_string(),
            short: None,
            long: Some("url".to_string()),
            help: Some("URL to scrape".to_string()),
            required: true,
            default: None,
        },
        arg_output(),
        arg_formats(),
        arg_timeout(),
        arg_strict_ssl(),
    ]
}

fn crawl_sub_args() -> Vec<PluginArgDef> {
    vec![
        PluginArgDef {
            name: "_positional".to_string(),
            short: None,
            long: None,
            help: Some("URL to crawl".to_string()),
            required: false,
            default: None,
        },
        PluginArgDef {
            name: "url".to_string(),
            short: None,
            long: Some("url".to_string()),
            help: Some("URL to crawl".to_string()),
            required: true,
            default: None,
        },
        arg_output(),
        PluginArgDef {
            name: "limit".to_string(),
            short: None,
            long: Some("limit".to_string()),
            help: Some("Maximum number of URLs to crawl".to_string()),
            required: false,
            default: Some("100".to_string()),
        },
        PluginArgDef {
            name: "max-depth".to_string(),
            short: None,
            long: Some("max-depth".to_string()),
            help: Some("Maximum crawl depth".to_string()),
            required: false,
            default: Some("10".to_string()),
        },
        PluginArgDef {
            name: "include-paths".to_string(),
            short: None,
            long: Some("include-paths".to_string()),
            help: Some("Regex patterns for paths to include (comma-separated)".to_string()),
            required: false,
            default: None,
        },
        PluginArgDef {
            name: "exclude-paths".to_string(),
            short: None,
            long: Some("exclude-paths".to_string()),
            help: Some("Regex patterns for paths to exclude (comma-separated)".to_string()),
            required: false,
            default: None,
        },
        PluginArgDef {
            name: "allow-subdomains".to_string(),
            short: None,
            long: Some("allow-subdomains".to_string()),
            help: Some("Allow crawling subdomains (true/false)".to_string()),
            required: false,
            default: Some("false".to_string()),
        },
        PluginArgDef {
            name: "sitemap".to_string(),
            short: None,
            long: Some("sitemap".to_string()),
            help: Some("Sitemap mode: skip, include, only".to_string()),
            required: false,
            default: Some("skip".to_string()),
        },
        PluginArgDef {
            name: "delay".to_string(),
            short: None,
            long: Some("delay".to_string()),
            help: Some("Delay between requests in ms (not implemented)".to_string()),
            required: false,
            default: Some("0".to_string()),
        },
        arg_formats(),
        arg_timeout(),
        arg_strict_ssl(),
    ]
}

fn arg_output() -> PluginArgDef {
    PluginArgDef {
        name: "output".to_string(),
        short: Some('o'),
        long: Some("output".to_string()),
        help: Some("Output path or directory".to_string()),
        required: false,
        default: Some("crawls".to_string()),
    }
}

fn arg_formats() -> PluginArgDef {
    PluginArgDef {
        name: "formats".to_string(),
        short: None,
        long: Some("formats".to_string()),
        help: Some("Output formats: markdown, rawHtml".to_string()),
        required: false,
        default: Some("markdown".to_string()),
    }
}

fn arg_timeout() -> PluginArgDef {
    PluginArgDef {
        name: "timeout".to_string(),
        short: None,
        long: Some("timeout".to_string()),
        help: Some("Request timeout in seconds".to_string()),
        required: false,
        default: None,
    }
}

fn arg_strict_ssl() -> PluginArgDef {
    PluginArgDef {
        name: "strict-ssl".to_string(),
        short: None,
        long: Some("strict-ssl".to_string()),
        help: Some("Use strict SSL (true/false)".to_string()),
        required: false,
        default: Some("true".to_string()),
    }
}

#[plugin_fn]
pub fn appz_plugin_execute(
    input: Json<PluginExecuteInput>,
) -> FnResult<Json<PluginExecuteOutput>> {
    let _command = &input.0.command;
    let args = &input.0.args;
    let working_dir = &input.0.working_dir;

    let vfs = WasmVfs;

    let pos = get_positional_vec(args);
    let subcommand = pos.first().and_then(|s| s.as_str());

    let (mode, url_arg) = if subcommand == Some("scrape") {
        ("scrape", pos.get(1).and_then(|v| v.as_str()).map(String::from))
    } else if subcommand == Some("crawl") {
        ("crawl", pos.get(1).and_then(|v| v.as_str()).map(String::from))
    } else {
        ("crawl", pos.first().and_then(|v| v.as_str()).map(String::from))
    };

    let url = match url_arg.or_else(|| str_arg(args, "url")) {
        Some(u) => u,
        None => {
            return Ok(Json(PluginExecuteOutput {
                exit_code: 1,
                message: Some("URL required. Use: appz crawl <url> or appz crawl scrape <url>".to_string()),
            }))
        }
    };

    let output = str_arg(args, "output").unwrap_or_else(|| "crawls".to_string());
    let output = to_vfs_path(working_dir, &output);
    let want_raw_html = str_arg(args, "formats")
        .map(|s| s.contains("rawHtml"))
        .unwrap_or(false);
    let strict_ssl = bool_arg(args, "strict-ssl").unwrap_or(true);

    match mode {
        "scrape" => {
            let out_path = if output.ends_with('/') || !output.contains('.') {
                format!("{}/page.md", output.trim_end_matches('/'))
            } else {
                output.clone()
            };
            match scrape::scrape_url(&vfs, &url, &out_path, want_raw_html, strict_ssl) {
                Ok(_) => Ok(Json(PluginExecuteOutput {
                    exit_code: 0,
                    message: Some(format!("Scraped {} -> {}", url, out_path)),
                })),
                Err(e) => Ok(Json(PluginExecuteOutput {
                    exit_code: 1,
                    message: Some(format!("Scrape failed: {}", e)),
                })),
            }
        }
        _ => {
            let limit = str_arg(args, "limit")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(100);
            let max_depth = str_arg(args, "max-depth")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(10);
            let include_paths = str_arg(args, "include-paths")
                .map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect())
                .unwrap_or_default();
            let exclude_paths = str_arg(args, "exclude-paths")
                .map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect())
                .unwrap_or_default();
            let allow_subdomains = bool_arg(args, "allow-subdomains").unwrap_or(false);
            let sitemap_mode = match str_arg(args, "sitemap").as_deref().unwrap_or("skip") {
                "include" => SitemapMode::Include,
                "only" => SitemapMode::Only,
                _ => SitemapMode::Skip,
            };

            let opts = CrawlOptions {
                base_url: url.clone(),
                output_dir: output,
                limit,
                max_depth,
                include_paths,
                exclude_paths,
                allow_subdomains,
                sitemap_mode,
                want_raw_html,
                strict_ssl,
            };

            match crawl::crawl(&vfs, &opts) {
                Ok(n) => Ok(Json(PluginExecuteOutput {
                    exit_code: 0,
                    message: Some(format!("Crawled {} pages", n)),
                })),
                Err(e) => Ok(Json(PluginExecuteOutput {
                    exit_code: 1,
                    message: Some(format!("Crawl failed: {}", e)),
                })),
            }
        }
    }
}

fn get_positional_vec(args: &HashMap<String, serde_json::Value>) -> Vec<serde_json::Value> {
    args.get("_positional")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

fn str_arg(args: &HashMap<String, serde_json::Value>, key: &str) -> Option<String> {
    args.get(key).and_then(|v| v.as_str()).map(String::from)
}

fn bool_arg(args: &HashMap<String, serde_json::Value>, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| {
        v.as_bool()
            .or_else(|| v.as_str().map(|s| s == "true" || s == "1"))
    })
}

fn to_vfs_path(working_dir: &str, path: &str) -> String {
    let path = path.trim().trim_start_matches("./");
    if path.is_empty() || path == "." {
        return ".".to_string();
    }
    if path.starts_with('/') {
        let wd = working_dir.trim_end_matches('/');
        if let Some(rel) = path.strip_prefix(wd) {
            return rel.trim_start_matches('/').to_string();
        }
    }
    path.to_string()
}
