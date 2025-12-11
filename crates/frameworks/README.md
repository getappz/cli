# Frameworks

A Rust crate that provides framework detection and configuration data, migrated from Vercel's frameworks TypeScript package.

## Architecture: PHF + Build-Time Code Generation

This crate uses an optimal architecture combining:

✅ **Maximum Performance**: Zero runtime allocations, all data uses `&'static str`  
✅ **Maximum Speed**: O(1) perfect-hash lookups via PHF  
✅ **Great Maintainability**: Data stored in JSON, code generated at build time  
✅ **Type Safety**: Compile-time verified, no runtime parsing  
✅ **No Dynamic Allocations**: Everything is statically allocated

## Usage

```rust
use frameworks::{frameworks, find_by_slug, find_by_name};

// Get all frameworks (returns a static slice - zero allocation)
let all_frameworks = frameworks();

// Find a framework by slug using O(1) perfect hash lookup
let nextjs = find_by_slug("nextjs").unwrap();
println!("{}", nextjs.name);

// Find a framework by name (linear search)
let gatsby = find_by_name("Gatsby.js");
```

## Performance Characteristics

### Runtime Costs: **ZERO**
- ✅ No JSON parsing at runtime
- ✅ No allocations
- ✅ No `String` creation
- ✅ Everything is `&'static str` stored in binary's read-only section

### Lookup Performance: **FASTEST POSSIBLE**
- ✅ **O(1) perfect hash lookups** via `phf::Map`
- ✅ Faster than `HashMap` (no collision handling needed)
- ✅ Immutable and thread-safe

### Build-Time Benefits
- ✅ Framework data is validated at compile time
- ✅ Type errors caught during build, not runtime
- ✅ Generated code is optimized by Rust compiler

## How It Works

1. **Source Data**: Framework metadata stored in `data/frameworks.json`
2. **Build Script**: `build.rs` reads JSON and generates Rust code
3. **Generated Code**: Static arrays and PHF maps created at compile time
4. **Runtime**: Zero-cost access to pre-compiled data

### Directory Structure

```
frameworks/
├── build.rs              # Code generator (runs at build time)
├── data/
│   └── frameworks.json   # Framework metadata (edit this!)
└── src/
    ├── types.rs          # Type definitions
    ├── frameworks.rs      # Public API + includes generated code
    └── lib.rs            # Module exports
```

## Adding/Editing Frameworks

Simply edit `data/frameworks.json` - the build script will automatically regenerate the Rust code on the next build.

Example framework entry:

```json
{
  "name": "Next.js",
  "slug": "nextjs",
  "website": "https://nextjs.org",
  "detectors": {
    "every": [{"matchPackage": "next"}]
  },
  "settings": {
    "buildCommand": {
      "value": "next build"
    }
  }
}
```

## Framework Structure

Each framework contains:
- **Metadata**: Name, slug, logo, description, website
- **Detectors**: How to detect if a project uses this framework (path, package, content matching)
- **Settings**: Build commands, dev commands, output directories
- **Runtime Configuration**: Runtime settings for deployment

## Supported Frameworks

- Blitz.js (Legacy)
- Next.js
- Gatsby.js
- Remix
- Other (catch-all)

## Migration Notes

This crate is a migration of the TypeScript code from:
https://github.com/vercel/vercel/blob/main/packages/frameworks/src/frameworks.ts

The original TypeScript file contains many more frameworks. Additional frameworks can be added by editing `data/frameworks.json` and rebuilding.

## Technical Details

- Uses `phf` (Perfect Hash Functions) for O(1) lookups
- All string data uses `&'static str` for zero allocations
- Build script generates optimized Rust code at compile time
- Generated code is stored in `target/` (gitignored)
