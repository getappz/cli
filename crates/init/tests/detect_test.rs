use init::detect::{parse_framework_blueprint, resolve_source};

#[test]
fn detect_framework_with_blueprint() {
    let (fw, bp) = parse_framework_blueprint("nextjs/ecommerce").unwrap();
    assert_eq!(fw, "nextjs");
    assert_eq!(bp, "ecommerce");
}

#[test]
fn detect_framework_default() {
    let resolved = resolve_source("nextjs").unwrap();
    assert_eq!(resolved.provider.slug(), "blueprint");
}

#[test]
fn detect_framework_slash_blueprint() {
    let resolved = resolve_source("nextjs/ecommerce").unwrap();
    assert_eq!(resolved.provider.slug(), "blueprint");
    assert_eq!(resolved.source, "nextjs/ecommerce");
}

#[test]
fn detect_git_escape_hatch() {
    let resolved = resolve_source("git:nextjs/my-template").unwrap();
    assert_eq!(resolved.provider.slug(), "git");
    assert_eq!(resolved.source, "nextjs/my-template");
}

#[test]
fn detect_non_framework_user_repo() {
    let resolved = resolve_source("someuser/somerepo").unwrap();
    assert_eq!(resolved.provider.slug(), "git");
}

#[test]
fn detect_wordpress() {
    let resolved = resolve_source("wordpress").unwrap();
    assert_eq!(resolved.provider.slug(), "blueprint");
}

#[test]
fn detect_npm_prefix() {
    let resolved = resolve_source("npm:create-foo").unwrap();
    assert_eq!(resolved.provider.slug(), "npm");
}

#[test]
fn detect_local_path() {
    let resolved = resolve_source("./my-project").unwrap();
    assert_eq!(resolved.provider.slug(), "local");
}

#[test]
fn detect_archive_url() {
    let resolved = resolve_source("https://example.com/template.zip").unwrap();
    assert_eq!(resolved.provider.slug(), "remote-archive");
}
