# export RUSTC_WRAPPER := "sccache"
set windows-shell := ["pwsh.exe", "-NoLogo", "-Command"]

init:
	cargo install cargo-binstall
	cargo binstall cargo-insta cargo-nextest cargo-llvm-cov

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

# OTHER

docs:
	cargo run -- run website:start

moon-check:
	cargo run -- check --all --log trace

schemas:
	cargo run -p moon_config