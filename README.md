# saasctl

A powerful task runner and deployment tool for modern web applications.

## Overview

saasctl is a Rust-based CLI tool that helps you manage tasks, run deployments, and orchestrate complex workflows for your projects. It provides:

- **Task Management**: Define and run tasks with dependencies
- **Recipe System**: Pre-built recipes for common deployment targets (Vercel, Netlify, Railway, etc.)
- **WASM Plugins**: Extensible plugin system using WebAssembly
- **Context Management**: Environment variables and context-aware execution
- **Self-Update**: Automatic updates via GitHub releases

## Quick Start

### Installation

```bash
# Using the install script
curl -fsSL https://raw.githubusercontent.com/your-org/saasctl/main/install.sh | bash

# Or download from GitHub Releases
# https://github.com/your-org/saasctl/releases
```

### Basic Usage

```bash
# Run a task
appz run build

# Link a project
appz link

# Deploy using a recipe
appz deploy vercel

# Check version and update
appz version
appz self-update
```

## Documentation

- **[User Documentation](docs/README.md)** - Complete user guides and references
- **[Developer Documentation](docs/developer/architecture.md)** - Architecture and contribution guides

## Project Structure

```
saasctl/
├── crates/          # Workspace crates
│   ├── app/        # Core application logic
│   ├── cli/        # CLI entry point
│   ├── api/        # API client
│   ├── task/       # Task execution engine
│   ├── ui/         # User interface components
│   └── ...
├── recipes/        # Deployment recipes
├── scripts/        # Build and release scripts
└── docs/           # Documentation
```

## Development

### Prerequisites

- Rust 1.70+ (edition 2021)
- Cargo

### Building

```bash
# Build the project
cargo build

# Build release binary
cargo build --release

# Run tests
cargo test
```

### Workspace Structure

This is a Cargo workspace with multiple crates. See [Cargo.toml](Cargo.toml) for workspace configuration.

## License

MIT

## Contributing

See [CONTRIBUTING.md](docs/developer/contributing.md) for development guidelines.

