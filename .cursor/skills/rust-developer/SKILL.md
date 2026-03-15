---
name: rust-developer
description: ALWAYS invoke this skill BEFORE writing or modifying ANY Rust code (.rs files) even for simple Hello World programs. Enforces Microsoft-style Rust development discipline and requires consulting the appropriate guideline files before any coding activity. This skill is MANDATORY for all Rust development.
---

**Current compliance date: 2026-02-18**

# Rust Development Skill

<!-- The Pragmatic Rust Guidelines are copyrighted (c) by Microsoft Corporation and licensed under the MIT license. -->

This skill enforces structured, guideline-driven Rust development. It ensures all Rust code strictly follows the appropriate Microsoft-style rules, documentation formats, and quality constraints.

## Mandatory Workflow

**This skill MUST be invoked for ANY Rust action**, including:

- Creating new `.rs` files (even minimal examples like Hello World)
- Modifying existing `.rs` files (any change, no matter how small)
- Reviewing, refactoring, or rewriting Rust code

## Which guideline to read and when

Before writing or modifying Rust code, **read ONLY the guideline files that apply to the requested task**, using segmented reading (`offset` and `limit`) when needed.

### Guidelines and when they apply

#### 1. `01_ai_guidelines.md`

Use when the Rust code involves:

- AI agents and LLM-driven code generation
- Making APIs easier for AI systems to use
- Comprehensive documentation and detailed examples
- Strong type systems that help AI avoid mistakes

#### 2. `02_application_guidelines.md`

Use when working on:

- Application-level error handling with anyhow or eyre
- CLI tools and desktop applications
- Performance optimization using mimalloc allocator
- User-facing features and initialization logic

#### 3. `03_documentation.md`

Use when:

- Writing public API documentation and doc comments
- Creating canonical documentation sections (Examples, Errors, Panics, Safety)
- Structuring module-level documentation comprehensively
- Re-exporting items and using `#[doc(inline)]` annotations

#### 4. `04_ffi_guidelines.md`

Use when:

- Loading multiple Rust-based dynamic libraries (DLLs)
- Creating FFI boundaries and interoperability layers
- Sharing data between different Rust compilation artifacts
- Dealing with portable vs non-portable data types across DLL boundaries

#### 5. `05_library_guidelines.md`

Use when creating or modifying **Rust libraries**, including:

- Structuring a crate
- Designing public APIs
- Dependency decisions

#### 6. `06_performance_guidelines.md`

Use when:

- Identifying and profiling hot paths in your code
- Optimizing for throughput and CPU cycle efficiency
- Managing allocation patterns and memory usage
- Implementing yield points in long-running async tasks

Also read [references/performance-optimizations.md](references/performance-optimizations.md) when:

- Using `regex::Regex` in hot paths (cache with `OnceLock`)
- Configuring release profiles (LTO, codegen-units, panic=abort)
- Adding benchmarks for parsing or validation code

#### 7. `07_safety_guidelines.md`

Use when:

- Writing unsafe code for novel abstractions, performance, or FFI
- Ensuring code soundness and preventing undefined behavior
- Documenting safety requirements and invariants
- Reviewing unsafe blocks for correctness with Miri

#### 8. `08_universal_guidelines.md`

Use in **ALL Rust tasks**. This file defines:

- General Rust best practices applicable to all code
- Style, naming, and organizational conventions
- Cross-cutting concerns that apply everywhere
- Foundational principles for any Rust project

#### 9. `09_libraries_building_guidelines.md`

Use when:

- Creating reusable library crates
- Managing Cargo features and their additivity
- Building native `-sys` crates for C interoperability
- Ensuring libraries work out-of-the-box on all platforms

#### 10. `10_libraries_interoperability_guidelines.md`

Use when:

- Exposing public APIs and managing external dependencies
- Designing types for Send/Sync compatibility
- Avoiding leaking third-party types from public APIs
- Creating escape hatches for native handle interop

#### 11. `11_libraries_resilience_guidelines.md`

Use when:

- Avoiding statics and thread-local state in libraries
- Making I/O and system calls mockable for testing
- Preventing glob re-exports and accidental leaks
- Feature-gating test utilities and mocking functionality

#### 12. `12_libraries_ux_guidelines.md`

Use when:

- Designing user-friendly library APIs
- Managing error types and error handling patterns
- Creating runtime abstractions and trait-based designs
- Structuring crate organization and public interfaces

## Rules (apply when working on `.rs` files)

This skill includes rules in `rules/`. Load and apply them for Rust work:

- [rust-docs.mdc](rules/rust-docs.mdc) – M-CANONICAL-DOCS format for public docs
- [rust-comments.mdc](rules/rust-comments.mdc) – American English for comments

## Coding Rules

1. **Load the necessary guideline files BEFORE ANY RUST CODE GENERATION.**
2. Apply the required rules from the relevant guidelines.
3. Apply the [M-CANONICAL-DOCS](03_documentation.md#M-CANONICAL-DOCS) documentation format (summary sentence < 15 words, then extended docs, Examples, Errors, Panics, Safety, Abort sections as applicable).
4. Comments must ALWAYS be written in American English, unless the user explicitly requests ‘write comments in French’ or provides another clear instruction specifying a different comment language.
5. If the file is fully compliant, add a comment: `// Rust guideline compliant {date}` where `{date}` is the **Current compliance date** shown at the top of this document.

## Fallback: full guidelines

If segmented reading is not suitable or a broad overview is needed, the complete guidelines are in [rust-guidelines.txt](rust-guidelines.txt). Use `Read` with `offset` and `limit` to avoid token limits (~25k tokens for the full file).

## Updating guidelines

Run from project root:

```bash
.cursor/skills/rust-developer/scripts/update-guidelines.sh
```

## Reference

- [Microsoft Pragmatic Rust Guidelines](https://microsoft.github.io/rust-guidelines/)
- [40tude: Microsoft Pragmatic Rust Guidelines with Claude](https://www.40tude.fr/docs/06_programmation/rust/019_ms_rust/ms_rust.html)
