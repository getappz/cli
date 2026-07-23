# Contributing to appz

Thanks for your interest in appz — contributions are welcome.

## Quick start

### Prerequisites

- Rust (stable) via [rustup](https://rustup.rs/)
- Git

### Setup

```bash
git clone https://github.com/getappz/cli.git
cd cli

cargo build --workspace
cargo test --workspace
```

### Quality bar (required)

Same checks CI runs on every PR:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings -A unsafe_code -A clippy::pedantic
cargo test --workspace
cargo deny check
```

## Repo structure

```text
cli/
├── crates/
│   ├── appz/          # the CLI binary — clap dispatch, MCP server, deploy glue
│   ├── appz-core/      # toolchain detection, mise.toml generation, doctor/report
│   ├── appz-deploy/    # DeployProvider trait + per-platform CLIs (Vercel, Netlify, ...)
│   └── command/        # shell-free process execution (arg-vector, timeouts)
└── .github/            # CI, templates, workflows
```

## Issues

- If your issue was closed but the problem persists, comment `/reopen` on it — as the original author, this reopens the issue automatically (GitHub itself doesn't let authors reopen maintainer-closed issues). Issues closed as *not planned* are a maintainer call and aren't reopened this way, but a comment is still welcome.

## Pull requests

- Keep PRs focused (one theme per PR)
- Include a short test plan (commands you ran)
- All tests must pass before merging

## Contributor License Agreement (CLA)

Before your first pull request can be merged, you need to sign our
[Contributor License Agreement](CLA.md). It is a one-time, automated step: the
CLA Assistant bot comments on your PR, and you sign by replying:

> I have read the CLA Document and I hereby sign the CLA

The CLA keeps appz Apache-2.0-licensed for everyone while allowing the
maintainer to relicense (e.g. for a hosted/commercial offering).

## License

appz is distributed under the Apache License, Version 2.0; by contributing,
your contributions are licensed to the public under the same terms (see the
[CLA](CLA.md) for the full grant).
