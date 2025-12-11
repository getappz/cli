# Framework Detectors

This module provides framework detection functionality migrated from Vercel's `detect-framework.ts`.

## Overview

The detectors module allows you to detect which web framework is being used in a project by analyzing the filesystem. It supports:

- **Package detection**: Detects frameworks by checking `package.json` for specific packages
- **Path detection**: Detects frameworks by checking for specific files/directories
- **Content detection**: Detects frameworks by matching regex patterns in file contents

## Architecture

### Filesystem Abstraction

The module uses a `DetectorFilesystem` trait to abstract filesystem operations, allowing for:
- Standard filesystem access (via `StdFilesystem`)
- Remote filesystem access (future)
- Virtual filesystem access (for testing)

### Detection Functions

1. **`detect_framework`**: Returns the slug of the first matching framework (legacy)
2. **`detect_frameworks`**: Returns all matching frameworks
3. **`detect_framework_record`**: Returns the first matching framework with version information

## Usage

```rust
use crate::detectors::{StdFilesystem, DetectFrameworkRecordOptions, detect_framework_record};
use frameworks::{frameworks, Framework};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // Create a filesystem detector
    let fs = Arc::new(StdFilesystem::new(Some("./my-project")));
    
    // Get all available frameworks
    let framework_list: Vec<Framework> = frameworks().to_vec();
    
    // Detect framework
    let options = DetectFrameworkRecordOptions {
        fs,
        framework_list,
    };
    
    match detect_framework_record(options).await {
        Ok(Some((framework, version))) => {
            println!("Detected framework: {}", framework.name);
            if let Some(v) = version {
                println!("Version: {}", v);
            }
        }
        Ok(None) => {
            println!("No framework detected");
        }
        Err(e) => {
            eprintln!("Error detecting framework: {}", e);
        }
    }
}
```

## Detector Types

### MatchPackage

Detects frameworks by checking if a package exists in `package.json`:

```json
{
  "detectors": {
    "every": [
      {
        "matchPackage": "next"
      }
    ]
  }
}
```

### Path

Detects frameworks by checking if a file/directory exists:

```json
{
  "detectors": {
    "some": [
      {
        "path": "next.config.js"
      },
      {
        "path": "next.config.ts"
      }
    ]
  }
}
```

### MatchContent

Detects frameworks by matching regex patterns in file contents:

```json
{
  "detectors": {
    "every": [
      {
        "path": "package.json",
        "matchContent": "\"next\"\\s*:"
      }
    ]
  }
}
```

## Supersedes Logic

Frameworks can declare that they "supersede" other frameworks. For example, Remix supersedes both "hydrogen" and "vite". When multiple frameworks match, the superseded ones are automatically removed from the results.

## Migration Notes

This module is a direct migration of:
- https://github.com/vercel/vercel/blob/main/packages/fs-detectors/src/detect-framework.ts

Key differences:
- Uses Rust's async/await instead of TypeScript promises
- Uses `Arc<dyn DetectorFilesystem>` for trait objects
- Returns `Result` types for error handling
- Uses regex crate for pattern matching

