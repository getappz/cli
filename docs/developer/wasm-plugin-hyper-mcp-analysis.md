# WASM Plugin Analysis: hyper-mcp Implementation

## Key Findings from hyper-mcp

After analyzing [hyper-mcp's implementation](https://github.com/tuananh/hyper-mcp/blob/main/src/plugins.rs), here are the critical insights:

### 1. **They Use Manifest Pattern**

```rust
use extism::{Manifest, Plugin, Wasm};

let manifest = Manifest::new([Wasm::data(wasm_content)]);
let plugin = Plugin::new(&manifest, [], true).unwrap();
```

**Key difference**: They use `Manifest::new([Wasm::data(wasm_content)])` instead of passing raw bytes to `Plugin::new()`.

### 2. **They Don't Use Host Functions**

Hyper-mcp passes **empty functions array `[]`** to `Plugin::new()`. Their plugins are self-contained and don't register host functions.

This means:
- Their plugins don't call back into the host
- They just export functions that the host calls
- No namespace matching issues

### 3. **Version Difference**

- hyper-mcp uses: `extism = "1.11.1"`
- We use: `extism = "1.12.0"`

This version difference might explain some incompatibilities.

### 4. **What We Can Learn**

1. **Use Manifest**: Always use `Manifest::new([Wasm::data(...)])` instead of raw bytes
2. **Pattern is correct**: Our function registration pattern looks correct
3. **Version compatibility**: May need to align extism versions

## Updated Implementation

We've updated our code to use Manifest pattern:

```rust
use extism::{Manifest, Plugin, Wasm, Function, ...};

let manifest = Manifest::new([Wasm::data(wasm_data)]);
let plugin = Plugin::new(&manifest, functions, true)?;
```

### Remaining Issue

Even with Manifest, we still get:
```
DEBUG: Plugin::new error: unknown import: `extism:host/user::saasctl_reg_register_task` has not been defined
```

This suggests:
1. **Version mismatch**: extism 1.12.0 might handle namespaces differently than extism-pdk 1.4.1 expects
2. **Function registration timing**: Maybe extism validates imports before registering functions
3. **Namespace format**: The actual WASM import might be different than we think

### Next Steps

1. **Check actual WASM imports**: Use `wasm-objdump` to see what the plugin actually imports
2. **Align versions**: Try `extism = "1.11.1"` to match hyper-mcp
3. **Verify namespace**: Check if extism-pdk 1.4.1 actually generates `extism:host/user::` or something else

## Conclusion

Hyper-mcp's implementation confirms:
- ✅ Using Manifest is correct
- ❌ They don't solve our host function problem (they don't use them)
- ⚠️ Version difference might be significant

The Manifest pattern is now in place, but we still need to solve the host function registration issue, likely through version alignment or namespace verification.

