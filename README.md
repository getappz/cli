<div align="center">

<pre>
+-+-+-+-+
|a|p|p|z|
+-+-+-+-+
</pre>

</div>

# appz

Zero-config project toolchain detection and lifecycle runner. `appz` detects
your stack (Node, Rust, Python, ...), wires up `mise`, and runs
install/build/dev/test/lint/format through one command, caching against
your project's inputs so repeat runs skip work that isn't stale.

```
appz init      # detect toolchains, scaffold appz.jsonc
appz install   # mise + package manager install
appz build     # run the detected (or overridden) build command
appz dev       # start the dev server
appz test      # run tests
appz lint      # run the linter
appz format    # run the formatter
appz doctor    # diagnose project stack, config, and suggestions
```

## Layout

- `crates/appz` — the CLI binary
- `crates/appz-core` — toolchain detection, `mise.toml` generation, and
  the doctor/report logic `appz` runs on top of

## Build

```
cargo build --release
```
