# WASM Plugin Troubleshooting

## Common Error: "unknown import: `extism:host/user::function_name` has not been defined"

This error occurs when the WASM plugin expects a host function that hasn't been registered, or there's a namespace/version mismatch.

### Root Cause Analysis

1. **Version Mismatch**: `extism-pdk` and `extism` versions must be compatible
2. **Namespace Format**: Different versions of extism-pdk use different namespace formats
3. **Function Registration**: Function name must exactly match what WASM imports

### Solution: Version Alignment

**Current Setup:**
- Host uses: `extism = "1.12.0"`
- Plugin uses: `extism-pdk = "1.4.1"` (originally) → updated to `"1.7"`

**Fix:**
1. Ensure `extism-pdk` version is compatible with `extism 1.12.0`
2. Rebuild the plugin after updating extism-pdk version
3. Ensure function names match exactly between host and plugin

### Verification Steps

1. **Check function registration**:
   ```rust
   // Host side - must match exactly what WASM imports
   Function::new(
       "extism:host/user::saasctl_reg_register_task",  // Exact match required
       [ValType::I64],
       [ValType::I32],
       UserData::new(host_data.clone()),
       register_task_host_fn,
   )
   ```

2. **Check plugin declaration**:
   ```rust
   // Plugin side - extism-pdk generates import automatically
   #[host_fn]
   extern "ExtismHost" {
       fn saasctl_reg_register_task(offset: u64) -> u32;
   }
   ```

3. **Rebuild plugin** after changing extism-pdk version:
   ```bash
   cd examples/plugins/hello
   cargo build --target wasm32-wasip1 --release
   ```

### Alternative: Use Bare Function Names (Warpgate Pattern)

If namespace issues persist, try matching warpgate's approach:

**Host:**
```rust
Function::new(
    "saasctl_reg_register_task",  // No namespace
    [ValType::I64],
    [ValType::I32],
    UserData::new(host_data.clone()),
    register_task_host_fn,
)
```

**Plugin:**
```rust
#[host_fn]
extern "ExtismHost" {
    #[link_name = "saasctl_reg_register_task"]  // Explicit link name
    fn saasctl_reg_register_task(offset: u64) -> u32;
}
```

### Debug Checklist

- [ ] Host function name matches WASM import exactly (check for typos, whitespace)
- [ ] extism and extism-pdk versions are compatible
- [ ] Plugin rebuilt after changing extism-pdk version
- [ ] Function signature matches (ValType::I64 input, ValType::I32 output)
- [ ] Functions array passed correctly to `Plugin::new()`

### Testing

Run with debug output to see what's happening:
```bash
cargo run --bin saasctl -- --plugin path/to/plugin.wasm list
```

The debug output should show:
- Which function is being registered
- How many functions are passed to Plugin::new
- Any errors during plugin creation

