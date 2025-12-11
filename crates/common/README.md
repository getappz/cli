# Common Crate

Shared utilities, constants, and helper functions used across the codebase.

## Modules

### Constants (`consts`)
Application constants, deployment-related constants, and common file/directory names.

### Path Utilities (`path`)
Path normalization, encoding, and cross-platform path manipulation utilities.

### ID Utilities (`id`)
ID generation, validation, and encoding/decoding helpers.

### Environment (`env`)
Environment variable helpers and platform detection utilities.

### Types (`types`)
Common type aliases and shared types.

## Usage

```rust
use common::{APP_NAME, normalize_path, generate_id};

// Use constants
println!("Application: {}", APP_NAME);

// Use path utilities
let normalized = normalize_path("/some/path");

// Use ID utilities
let id = generate_id("my-resource");
```

