# WASM Plugin Fix Summary

## Root Cause Analysis

After analyzing fluentci-engine and moonrepo/warpgate patterns, the issue was identified:

1. **extism-pdk 1.4.1 automatically adds namespace prefix**: When using `#[host_fn]` in extism-pdk 1.4.1, the WASM module imports functions with the `extism:host/user::` prefix automatically.

2. **Host must match exact import name**: The host function registration must use the exact name that the WASM module imports, including the namespace.

3. **Version mismatch**: There's a potential version mismatch between:
   - Host: `extism = "1.12.0"`
   - Plugin: `extism-pdk = "1.4.1"`

## The Fix

### Host Side (`crates/cli/src/wasm/plugin.rs`)

```rust
// extism-pdk 1.4.1 automatically adds "extism:host/user::" namespace to #[host_fn] functions
// We must register with the exact namespace that extism-pdk generates
let fn_name = "extism:host/user::saasctl_reg_register_task";

let reg_fn = Function::new(
    fn_name,
    [ValType::I64],
    [ValType::I32],
    UserData::new(host_data.clone()),
    register_task_host_fn,
);
```

### Plugin Side (`examples/plugins/hello/src/lib.rs`)

```rust
// extism-pdk 1.4.1 automatically prefixes with "extism:host/user::"
// The host must register with the exact namespace that extism-pdk generates
#[host_fn]
extern "ExtismHost" {
    fn saasctl_reg_register_task(offset: u64) -> u32;
}
```

## Key Insights from fluentci-engine

1. **fluentci-engine uses extism-pdk** for plugins but doesn't show their host function registration in the public docs
2. **moonrepo/warpgate uses bare function names** without namespace prefixes - they don't use `extism:host/user::`
3. **Both patterns work** - the key is consistency between host and plugin

## Next Steps

1. **Verify version compatibility**: Ensure `extism-pdk 1.4.1` is compatible with `extism 1.12.0`
2. **Test with matching versions**: Try updating both to latest compatible versions
3. **Alternative approach**: If namespace continues to cause issues, adopt moonrepo's bare name pattern:
   - Host registers: `"saasctl_reg_register_task"` (no namespace)
   - Plugin uses: `#[link_name = "saasctl_reg_register_task"]` to override default namespace

## Current Error

Even with the namespace fix, we're still getting:
```
DEBUG: Plugin::new error: unknown import: `extism:host/user::saasctl_reg_register_task` has not been defined
```

This suggests:
- The function registration isn't working as expected
- Or there's a deeper version incompatibility
- Or `Plugin::new()` API changed in extism 1.12.0

## Recommended Action

1. Check if extism-pdk 1.4.1 is actually compatible with extism 1.12.0
2. If not, update to compatible versions
3. Consider adopting moonrepo's bare name pattern for better compatibility

