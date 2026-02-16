# export RUSTC_WRAPPER := "sccache"
set windows-shell := ["pwsh.exe", "-NoLogo", "-Command"]

init:
	cargo install cargo-binstall
	cargo binstall cargo-insta cargo-nextest cargo-llvm-cov cargo-machete cargo-set-version

# BUILDING

build:
	cargo build -p cli
	sudo cp ./target/debug/appz ~/.local/bin/appz
build-rel:
	cargo build -p cli --release
	sudo cp ./target/release/appz ~/.local/bin/appz
build-pageflare:
	cargo build -p pageflare
	sudo cp ./target/debug/pageflare ~/.local/bin/pageflare

build-win:
	Set-Item -Path Env:RUSTC_WRAPPER -Value "sccache"
	cargo build -p appz_cli
	Copy-Item -Path .\target\debug\appz.exe -Destination $HOME\.appz\bin\appz.exe

build-ss-win:
	cargo build -p sitescrape
	Copy-Item -Path .\target\debug\sitescrape.exe -Destination $HOME\.appz\bin\sitescrape.exe

build-pf-win:
	cargo build -p pageflare
	Copy-Item -Path .\target\debug\pageflare.exe -Destination $HOME\.appz\bin\pageflare.exe

build-wasm:
	cd wasm/test-plugin && cargo wasi build

# CHECKING

check:
	cargo check --workspace

format:
	cargo fmt --all -- --emit=files

format-check:
	cargo fmt --all --check

lint:
	cargo clippy --all-targets --all-features

lint-workspace:
	cargo clippy --workspace --all-targets

lint-fix:
	cargo clippy --workspace --fix --allow-dirty --allow-staged

# Find unused dependencies across the workspace.
# Run `cargo install cargo-machete` first if not installed.
machete:
	cargo machete --with-metadata 

# Pre-commit hook
install-pre-commit:
	bash ./scripts/install-pre-commit.sh

# TESTING

test $MOON_TEST="true" name="":
	cargo nextest run --workspace --config-file ./.cargo/nextest.toml {{name}}

test-ci $MOON_TEST="true":
	cargo nextest run --workspace --config-file ./.cargo/nextest.toml --profile ci

# CODE COVERAGE

cov:
	cargo llvm-cov nextest --workspace --config-file ./.cargo/nextest.toml --profile ci

gen-report:
	cargo llvm-cov report --lcov --ignore-filename-regex error --output-path ./report.txt

gen-html:
	cargo llvm-cov report --html --ignore-filename-regex error --open

# RELEASING

bump type="patch":
	bash ./scripts/version/bumpBinaryVersions.sh {{type}}

bump-all:
	bash ./scripts/version/forceBumpAllVersions.sh

bump-interactive:
	yarn version check --interactive

release:
	node ./scripts/version/applyAndTagVersions.mjs

# PLUGINS

# Build + inject + sign plugins (output to dist/plugins)
plugin-package plugin="" no_wasm_opt="false":
	cargo run -p plugin-build -- package --output dist/plugins {{if plugin != "" { "--plugin " + plugin } else { "" }}} {{if no_wasm_opt == "true" { "--no-wasm-opt" } else { "" }}}

# Package + upload to CDN (requires S3/R2 env vars)
plugin-publish plugin="" dry_run="false" no_wasm_opt="false":
	cargo run -p plugin-build -- package --output dist/plugins {{if plugin != "" { "--plugin " + plugin } else { "" }}} {{if no_wasm_opt == "true" { "--no-wasm-opt" } else { "" }}}
	cargo run -p plugin-build -- publish --output dist/plugins {{if plugin != "" { "--plugin " + plugin } else { "" }}} {{if dry_run == "true" { "--dry-run" } else { "" }}}

# Full release: bump version, package, publish (requires cargo set-version for bump)
plugin-release plugin="" bump="" dry_run="false" no_wasm_opt="false":
	cargo run -p plugin-build -- release --output dist/plugins {{if plugin != "" { "--plugin " + plugin } else { "" }}} {{if bump != "" { "--bump " + bump } else { "" }}} {{if dry_run == "true" { "--dry-run" } else { "" }}} {{if no_wasm_opt == "true" { "--no-wasm-opt" } else { "" }}}

# OTHER

docs:
	cargo run -- run website:start

moon-check:
	cargo run -- check --all --log trace

schemas:
	cargo run -p moon_config