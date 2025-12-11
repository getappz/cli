# WASM Plugin Analysis: webxtism Implementation

## Key Findings

After analyzing webxtism's implementation, here are the key insights:

### 1. **Different Runtime, Similar Patterns**

Webxtism uses **Wasmer** runtime, not extism's runtime. However, their patterns provide insights:

- They use `(namespace, name)` tuple matching: `(dependency_name, import.name())`
- Default namespace: `EXTISM_USER_MODULE = "extism:host/user"`
- Functions are registered with explicit namespace support via builder pattern

### 2. **Function Registration Pattern**

```rust
// Webxtism pattern (Wasmer)
HostExportBuilder::new("hello_world")
    .namespace("extism:host/user")  // Optional, defaults to "extism:host/user"
    .function_in_out(hello_world, ())
```

They match imports using:
```rust
external_imports.get(&(dependency_name.to_owned(), import.name().to_owned()))
```

### 3. **Extism Runtime Differences**

Since we're using extism (not Wasmer), we use:
```rust
Function::new(
    "extism:host/user::saasctl_reg_register_task",  // Full name including namespace
    [ValType::I64],
    [ValType::I32],
    UserData::new(host_data.clone()),
    register_task_host_fn,
)
```

### 4. **Version Compatibility Issue**

**Critical Finding:**
- Host uses: `extism = "1.12.0"`
- Plugin uses: `extism-pdk = "1.4.1"`

When compiling the plugin, Cargo resolves:
- `extism-manifest v1.12.0` (from extism-pdk 1.4.1 dependencies)
- `extism-convert v1.12.0` (from extism-pdk 1.4.1 dependencies)

This suggests **extism-pdk 1.4.1 might not be fully compatible with extism 1.12.0**.

### 5. **The Solution**

The function name format is correct: `"extism:host/user::saasctl_reg_register_task"`

The issue is likely:
1. **Version mismatch**: extism-pdk 1.4.1 may generate WASM imports incompatible with extism 1.12.0
2. **API changes**: extism 1.12.0 might have changed how `Plugin::new` validates/registers functions

### Recommended Fix

1. **Update extism-pdk** to a version compatible with extism 1.12.0:
   ```toml
   extism-pdk = "1.7"  # or whatever matches extism 1.12.0
   ```

2. **Or downgrade extism** to match extism-pdk 1.4.1:
   ```toml
   extism = "1.4.1"  # Match extism-pdk version
   ```

3. **Verify the actual WASM imports**: Use `wasm-objdump` or similar to check what the plugin actually imports:
   ```bash
   wasm-objdump -x hello_plugin.wasm | grep import
   ```

### Key Takeaways

- Webxtism's patterns show namespace handling, but they use Wasmer
- Our implementation is correct (using full namespace in function name)
- The issue is likely version incompatibility between extism and extism-pdk
- Need to ensure extism and extism-pdk versions match

