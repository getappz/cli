# Hello Plugin

A simple test plugin for saasctl that demonstrates basic plugin functionality.

## Building

```bash
cargo build --target wasm32-wasip1 --release
```

The compiled WASM file will be at:
`target/wasm32-wasip1/release/hello_plugin.wasm`

## Testing

1. Build the plugin (see above)
2. Run saasctl with the plugin:

```bash
cargo run -- --plugin target/wasm32-wasip1/release/hello_plugin.wasm list
```

This should show the `hello:world` task registered by the plugin.

