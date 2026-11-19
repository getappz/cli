# Changelog

## [0.3.0] - 2026-11-19

### Features

- Deploy sends BLAKE3 (`b3:`) content hashes; the appz.dev API now requires them (#81)
- Fingerprint appz.dev static assets with immutable aliases (#81)
- appz.jsonc Markdown opt-in and hosted Markdown siblings (#79)
- Parallel `--all` deploys with per-target results, hook timeout and deploy state lock (#68)
- Skip hidden dirs and honor `.appzignore` / `.vercelignore` (#75)
- Wire source backups into the CLI (#74)
- Static HTML dev, preview and deploy support (#70, #72)
- Top-level `appz vault` subcommand (#69)
- `appz share` end to end (#67)
- Show fallback DNS guidance for custom domains (#77)

### Bug Fixes

- Follow the project lockfile for every JS toolchain's install command (#76)
- Install missing dependencies before `appz dev` (#73)
- Bump quinn-udp and rustix for cargo audit (#71)

## [0.2.0] - 2026-07-24

### Features

- Add appz devcontainer Feature (#13)
- Add GitHub Action wrapping appz CLI via npm install (#11)

### Bug Fixes

- Astro (and other npm-local CLIs) not found by mise/build tasks on Windows (#14)
- Correct npm package name in README (#12)
- Publish npm main package as @getappz/cli (#10)
- SLSA provenance pin + README release-channel docs (#9)

### Miscellaneous Tasks

- Disable PPA release trigger for now (#7)
- Scoop manifest: appz 0.1.0 (#8)
