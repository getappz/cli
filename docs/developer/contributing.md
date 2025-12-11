# Contributing Guide

Thank you for contributing to saasctl! This guide will help you get started.

## Development Setup

### Prerequisites

- **Rust**: Latest stable version (install via [rustup.rs](https://rustup.rs/))
- **Cargo**: Comes with Rust
- **Git**: For version control

### Getting Started

```bash
# Clone repository
git clone <repository-url>
cd saasctl

# Build
cargo build

# Run tests
cargo test

# Run linter
cargo clippy

# Format code
cargo fmt
```

## Project Structure

```
saasctl/
├── crates/
│   ├── cli/          # CLI binary and recipes
│   ├── task/         # Core task runner
│   └── command/      # Command execution utilities
├── docs/             # Documentation
└── Cargo.toml        # Workspace configuration
```

## Code Style

### Formatting

Use `cargo fmt` to format code:

```bash
cargo fmt
```

### Linting

Use `cargo clippy` to catch issues:

```bash
cargo clippy -- -D warnings
```

### Style Guidelines

1. **Use meaningful names**: Variables, functions, and types should be self-documenting
2. **Add documentation**: Public APIs should have doc comments
3. **Keep functions focused**: Each function should do one thing
4. **Handle errors properly**: Use `Result` types and `?` operator
5. **Use async appropriately**: Async for I/O, sync for CPU-bound work

### Example

```rust
/// Executes a deployment task.
///
/// # Arguments
///
/// * `ctx` - Context containing deployment configuration
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if deployment fails.
pub async fn deploy(ctx: &Context) -> TaskResult {
    let path = ctx.get("deploy_path")
        .ok_or_else(|| miette!("deploy_path must be set"))?;
    
    // Deployment logic...
    
    Ok(())
}
```

## Development Workflow

### 1. Create a Branch

```bash
git checkout -b feature/my-feature
# or
git checkout -b fix/my-bug
```

### 2. Make Changes

- Write code following style guidelines
- Add tests for new functionality
- Update documentation

### 3. Test Your Changes

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Check for issues
cargo clippy
cargo fmt --check
```

### 4. Commit Changes

```bash
git add .
git commit -m "Add feature: description"
```

Commit messages should:
- Be clear and descriptive
- Use present tense ("Add feature" not "Added feature")
- Reference issues if applicable ("Fix #123")

### 5. Push and Create PR

```bash
git push origin feature/my-feature
```

Then create a pull request with:
- Description of changes
- Link to related issues
- Screenshots/examples if applicable

## Adding Features

### New Recipe

1. Create file: `crates/cli/src/recipe/my_recipe.rs`
2. Implement `register_my_recipe(reg: &mut TaskRegistry)`
3. Add to module: `crates/cli/src/recipe/mod.rs`
4. Register in main: `crates/cli/src/main.rs`
5. Add tests
6. Document in user guide

### New Task Feature

1. Add to `Task` struct if needed
2. Update builder methods
3. Update runner to handle new feature
4. Add tests
5. Update documentation

### New Context Feature

1. Add to `Context` struct
2. Implement methods
3. Ensure thread safety
4. Add tests
5. Document API

## Writing Tests

See [Testing Guide](testing.md) for details.

### Test Coverage

- Unit tests for individual functions
- Integration tests for recipes
- Edge cases and error conditions
- Namespace scoping behavior

### Running Tests

```bash
# All tests
cargo test

# With output
cargo test -- --nocapture

# Specific test
cargo test test_name

# Specific crate
cargo test -p task
```

## Documentation

### Code Documentation

- Use `///` for public API documentation
- Include examples in doc comments
- Document parameters and return values

### User Documentation

Update `docs/user/` when adding user-facing features:
- Getting started guide
- User guide
- Recipes guide
- Advanced features

### Developer Documentation

Update `docs/developer/` when changing internals:
- Architecture
- API reference
- Extending guide
- Contributing guide

## Code Review

### Before Submitting

- [ ] Code compiles without warnings
- [ ] All tests pass
- [ ] Code is formatted (`cargo fmt`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Documentation updated
- [ ] Tests added for new features

### Review Checklist

- Code is clear and maintainable
- Follows project conventions
- Handles errors appropriately
- Tests cover new functionality
- Documentation is complete
- No breaking changes (or documented)

## Issue Reporting

### Bug Reports

Include:
- Description of the bug
- Steps to reproduce
- Expected behavior
- Actual behavior
- Environment (OS, Rust version)
- Relevant code/logs

### Feature Requests

Include:
- Description of the feature
- Use case / motivation
- Proposed solution (if any)
- Alternatives considered

## Release Process

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Tag release: `git tag v0.1.0`
4. Push tag: `git push origin v0.1.0`
5. Create GitHub release

## Questions?

- Open an issue for questions
- Check existing documentation
- Review code examples in recipes

## License

By contributing, you agree that your contributions will be licensed under the same license as the project.

