//! Test that SkillsCommands::Add parses --code.

use clap::Parser;
use skills::SkillsCommands;

#[derive(Parser)]
struct TestCli {
    #[command(subcommand)]
    command: TestCommands,
}

#[derive(clap::Subcommand)]
enum TestCommands {
    Skills {
        #[command(subcommand)]
        command: SkillsCommands,
    },
}

#[test]
fn add_parses_code_flag() {
    let cli = TestCli::try_parse_from(&["appz", "skills", "add", "--code", "--list"])
        .expect("parse should succeed");
    let TestCommands::Skills { command } = cli.command else {
        panic!("expected Skills");
    };
    let SkillsCommands::Add {
        code,
        source,
        list,
        ..
    } = command
    else {
        panic!("expected Add");
    };
    assert!(code, "--code should be true");
    assert_eq!(source, None);
    assert!(list, "--list should be true");
}

#[test]
fn add_parses_code_compress_and_directories() {
    let cli = TestCli::try_parse_from(&[
        "appz",
        "skills",
        "add",
        "--code",
        "--compress",
        "--directories",
        "src",
        "lib",
    ])
    .expect("parse should succeed");
    let TestCommands::Skills { command } = cli.command else {
        panic!("expected Skills");
    };
    let SkillsCommands::Add {
        code,
        compress,
        directories,
        ..
    } = command
    else {
        panic!("expected Add");
    };
    assert!(code, "--code should be true");
    assert!(compress, "--compress should be true");
    assert_eq!(
        directories.as_ref().map(|d| d.len()),
        Some(2),
        "directories should have 2 entries"
    );
}
