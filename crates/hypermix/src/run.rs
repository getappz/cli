//! Main run flow: load config, run repomix per mix, update ignores, print tokens.

use std::path::Path;

use crate::ignore::update_ignore_files;
use crate::load_config;
use crate::repomix::run_mix;
use crate::tokens::count_tokens_in_files;

pub async fn run(cwd: &Path, config_path: Option<&Path>) -> miette::Result<()> {
    let (_config_path, config) = load_config(config_path, cwd)?;
    let output_path = config.output_path.as_deref().unwrap_or(".hypermix");
    let output_dir = cwd.join(output_path);

    let mut results = Vec::new();
    for mix in &config.mixes {
        if let Some(r) = run_mix(cwd, mix, &output_dir).await? {
            if r.output_path.exists() {
                results.push(r.output_path);
            }
        }
    }

    if results.is_empty() {
        return Err(miette::miette!(
            "No valid output files created. Check config and ensure repomix is available (npx repomix@latest --version)"
        ));
    }

    update_ignore_files(cwd, &output_dir)?;

    let (file_tokens, total) = count_tokens_in_files(&results)?;

    if !config.silent.unwrap_or(false) {
        eprintln!("┌────────────────┬─────────────┐");
        eprintln!("│ File           │ Tokens      │");
        eprintln!("├────────────────┼─────────────┤");
        for (name, tokens) in &file_tokens {
            eprintln!("│ {:14} │ {:>11} │", name, format_number(*tokens));
        }
        eprintln!("└────────────────┴─────────────┘");
        eprintln!("Total Tokens: {}", format_number(total));

        let high = file_tokens.iter().any(|(_, t)| *t >= 60_000);
        if high {
            eprintln!("⚠️  One or more files exceed 60k tokens. Consider adding --compress to config.");
        }
    }

    Ok(())
}

fn format_number(n: u64) -> String {
    n.to_string()
        .chars()
        .rev()
        .collect::<Vec<_>>()
        .chunks(3)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(",")
        .chars()
        .rev()
        .collect()
}
