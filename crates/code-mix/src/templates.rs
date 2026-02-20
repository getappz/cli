//! Built-in prompt templates for pack output.
//!
//! Used with Repomix --instruction-file-path to prepend AI instructions.

/// Built-in template content (plain text, no handlebars).
pub static TEMPLATES: &[(&str, &str)] = &[
    (
        "review",
        r#"Review the following code for:

1. **Code Quality**
   - Readability and maintainability
   - Naming conventions
   - Code organization

2. **Potential Issues**
   - Bugs or logic errors
   - Edge cases not handled
   - Performance concerns

3. **Best Practices**
   - Design patterns usage
   - Error handling
   - Security considerations

4. **Suggestions**
   - Improvements to consider
   - Refactoring opportunities

Please provide specific, actionable feedback with code examples where appropriate."#,
    ),
    (
        "tests",
        r#"Write comprehensive unit tests for the following code.

Requirements:
- Test all public functions and methods
- Include edge cases and boundary conditions
- Mock external dependencies appropriately
- Aim for >80% code coverage
- Use descriptive test names that explain the expected behavior

Structure:
- Group related tests using describe blocks
- Use beforeEach/afterEach for setup/teardown where needed
- Include both positive and negative test cases"#,
    ),
    (
        "refactor",
        r#"Analyze the following code and suggest refactoring improvements.

Focus areas:
1. **Code Smells** - Long methods, duplicated code, complex conditionals, deep nesting
2. **Design Improvements** - SRP, dependency injection, interface extraction
3. **Modernization** - Modern language features, deprecated patterns, performance

For each suggestion: explain WHY, show BEFORE/AFTER, note trade-offs."#,
    ),
    (
        "explain",
        r#"Explain the following code in detail.

Please cover:
1. **Overview** - What does this code do? What problem does it solve?
2. **Key Components** - Main functions/classes, data structures, algorithms
3. **Flow** - How does data flow? What is the execution sequence?
4. **Dependencies** - External libraries used and why

Use clear, beginner-friendly language while being technically accurate."#,
    ),
    (
        "bugs",
        r#"Analyze the following code for potential bugs and issues.

Check for:
1. **Logic Errors** - Off-by-one, incorrect comparisons, infinite loops
2. **Null/Undefined Issues** - Null dereferences, uninitialized variables
3. **Resource Management** - Memory leaks, unclosed resources, race conditions
4. **Error Handling** - Unhandled exceptions, swallowed errors
5. **Security Vulnerabilities** - Injection risks, unsafe data handling

For each issue: describe the bug, show problematic code, provide a fix."#,
    ),
    (
        "security",
        r#"Perform a security audit on the following code.

Check for:
1. **Injection Vulnerabilities** - SQL, command, XSS, template injection
2. **Authentication & Authorization** - Weak auth, missing checks, session issues
3. **Data Protection** - Sensitive data exposure, insecure storage
4. **Input Validation** - Missing sanitization, type confusion
5. **Configuration** - Hardcoded secrets, debug mode, insecure defaults

Rate findings by severity (Critical/High/Medium/Low) and provide remediation."#,
    ),
    (
        "document",
        r#"Generate comprehensive documentation for the following code.

Include:
1. **Module Overview** - Purpose, responsibility, key concepts
2. **API Documentation** - Signatures, parameters, returns, exceptions
3. **Usage Examples** - Basic usage, common patterns, edge cases
4. **Dependencies** - Required imports, environment setup

Follow documentation best practices for this language/framework."#,
    ),
    (
        "optimize",
        r#"Analyze the following code for performance optimization opportunities.

Focus on:
1. **Time Complexity** - Algorithm efficiency, loop optimizations, caching
2. **Space Complexity** - Memory usage, data structure choices
3. **I/O Performance** - Database queries, network calls, file operations
4. **Concurrency** - Parallelization, async/await, thread safety

For each optimization: explain bottleneck, provide solution, estimate improvement."#,
    ),
];

/// Get template content by name. Returns None if not found.
pub fn get(name: &str) -> Option<&'static str> {
    TEMPLATES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, content)| *content)
}

/// List all template names.
pub fn list() -> impl Iterator<Item = &'static str> {
    TEMPLATES.iter().map(|(name, _)| *name)
}
