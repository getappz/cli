# WASM Plugin Debugging Findings

## WASM Import Analysis

Using `wasm-tools print`, we discovered the actual WASM import:

```
(import "extism:host/user" "saasctl_reg_register_task" ...)
```

**Key Finding**: The WASM imports use TWO separate strings:
- Module: `"extism:host/user"`
- Function name: `"saasctl_reg_register_task"`

This is NOT `"extism:host/user::saasctl_reg_register_task"` as a single string!

## Current Registration Attempts

1. **Full namespace path**: `"extism:host/user::saasctl_reg_register_task"`
   - Error: `unknown import: 'extism:host/user::saasctl_reg_register_task' has not been defined`
   - extism doesn't match this format

2. **Bare function name**: `"saasctl_reg_register_task"`
   - Error: `incompatible import type for 'extism:host/user::saasctl_reg_register_task'`
   - extism finds the import but can't match our registration

## The Problem

extism's `Function::new()` API appears to expect a different format than what the WASM actually imports, OR there's a mismatch in how extism matches functions to WASM imports.

## Possible Solutions

1. **Check extism API documentation** for how to register namespaced functions
2. **Try a different function name format** (maybe without `::` separator)
3. **Check if extism version mismatch** causes different behavior
4. **Look for extism examples** that actually use host functions with namespaces

## Next Steps

- Verify if extism's Function::new supports namespace registration differently
- Check extism source code for how it matches imports to registered functions
- Try alternative registration patterns

