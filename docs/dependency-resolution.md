# Dependency Resolution in Kam

Kam provides an advanced dependency resolution system inspired by Python's PEP 735 dependency groups and the uv package manager. This system allows you to organize dependencies into groups, include groups within other groups, and automatically resolve all dependencies with proper cycle detection.

## Features

### 1. Dependency Groups

Dependencies in Kam are organized into groups. The two main groups are:

- **normal**: Regular runtime dependencies
- **dev**: Development dependencies

You can define these groups in your `kam.toml` file:

```toml
[kam.dependency]
normal = [
    { id = "base-module", version = "1.0.0" },
    { id = "utils", version = "2.0.0" }
]

dev = [
    { id = "test-framework", version = "3.0.0" },
    { id = "linter", version = "1.5.0" }
]
```

### 2. Group Includes

Groups can include other groups using the special `include:` prefix. This allows you to build hierarchical dependency structures:

```toml
[kam.dependency]
# Base dependencies
normal = [
    { id = "core-lib", version = "1.0.0" },
    { id = "common-utils", version = "2.0.0" }
]

# Development dependencies include all normal dependencies
dev = [
    { id = "include:normal" },  # Include all normal dependencies
    { id = "test-framework", version = "3.0.0" },
    { id = "debug-tools", version = "1.5.0" }
]
```

When resolved, the `dev` group will contain:
- `core-lib` (1.0.0)
- `common-utils` (2.0.0)
- `test-framework` (3.0.0)
- `debug-tools` (1.5.0)

### 3. Nested Includes

You can create multi-level dependency hierarchies:

```toml
[kam.dependency]
# Base level
base = [
    { id = "fundamental-lib", version = "1.0.0" }
]

# Middle level includes base
runtime = [
    { id = "include:base" },
    { id = "runtime-lib", version = "2.0.0" }
]

# Top level includes runtime (which includes base)
dev = [
    { id = "include:runtime" },
    { id = "test-lib", version = "3.0.0" }
]
```

The `dev` group will resolve to:
- `fundamental-lib` (1.0.0)
- `runtime-lib` (2.0.0)
- `test-lib` (3.0.0)

### 4. Cycle Detection

Kam automatically detects circular dependencies and prevents infinite loops:

```toml
# This will produce an error!
[kam.dependency]
group-a = [
    { id = "include:group-b" }
]

group-b = [
    { id = "include:group-a" }
]
```

Error message:
```
Cycle detected in dependency groups: `group-a` -> `group-b` -> `group-a`
```

### 5. Validation

Before resolving dependencies, you can validate that all group includes reference existing groups:

```toml
# This will fail validation
[kam.dependency]
normal = [
    { id = "include:nonexistent" }  # Error: group 'nonexistent' not found
]
```

Error message:
```
Dependency group 'nonexistent' not found (referenced by 'normal')
```

## Usage in Code

### Resolving Dependencies

```rust
use kam::types::kam_toml::KamToml;

// Load kam.toml
let kam = KamToml::load_from_dir(Path::new("."))?;

// Resolve all dependencies
let resolved = kam.resolve_dependencies()?;

// Get dependencies for a specific group
if let Some(normal_group) = resolved.get("normal") {
    for dep in &normal_group.dependencies {
        println!("Dependency: {} ({})", 
            dep.id, 
            dep.version.as_ref().unwrap_or(&"*".to_string())
        );
    }
}
```

### Validating Dependencies

```rust
use kam::types::kam_toml::KamToml;

// Load kam.toml
let kam = KamToml::load_from_dir(Path::new("."))?;

// Validate before resolving
if let Err(e) = kam.validate_dependencies() {
    eprintln!("Invalid dependency configuration: {}", e);
    return Err(e.into());
}

// Now safe to resolve
let resolved = kam.resolve_dependencies()?;
```

## Error Handling

The dependency resolver provides detailed error messages for various failure modes:

### Group Not Found
```
Dependency group 'missing-group' not found (referenced by 'dev')
```

### Cycle Detected
```
Cycle detected in dependency groups: `group-a` -> `group-b` -> `group-c` -> `group-a`
```

### Invalid Include Syntax
```
Invalid include syntax in group 'dev': 'invalid-spec'
```

## Best Practices

1. **Use includes for hierarchical dependencies**: If your development environment needs all runtime dependencies, use `include:normal` in your dev group.

2. **Keep groups focused**: Each group should represent a specific use case (runtime, testing, development, etc.).

3. **Validate early**: Call `validate_dependencies()` before `resolve_dependencies()` to catch configuration errors early.

4. **Document your groups**: Use comments in your `kam.toml` to explain why certain dependencies are grouped together.

5. **Avoid deep nesting**: While nested includes are supported, try to keep your dependency hierarchy shallow for maintainability.

## Example Configuration

Here's a complete example showing various dependency resolution features:

```toml
[prop]
id = "my-module"
name = { en = "My Module" }
version = "1.0.0"
versionCode = 1
author = "Your Name"
description = { en = "A sample module" }

[mmrl]
zip_url = "https://example.com/module.zip"
changelog = "https://example.com/changelog.md"

[kam]
module_type = "Normal"

# Core runtime dependencies
[kam.dependency]
normal = [
    { id = "base-framework", version = ">=1.0.0, <2.0.0" },
    { id = "utils", version = "2.5.0" },
]

# Testing dependencies include normal plus test tools
test = [
    { id = "include:normal" },
    { id = "test-framework", version = "3.0.0" },
    { id = "mock-library", version = "1.2.0" },
]

# Full development environment includes everything
dev = [
    { id = "include:test" },
    { id = "linter", version = "4.0.0" },
    { id = "formatter", version = "2.1.0" },
]
```

## Implementation Details

The dependency resolver is implemented in the `dependency_resolver` module and follows these principles:

1. **Recursive resolution**: Groups are resolved recursively, following all includes.
2. **Cycle prevention**: A visited set tracks the resolution path to detect cycles.
3. **Error context**: Errors include information about which group caused the problem.
4. **Immutability**: Resolved groups are immutable once created.

The implementation is inspired by the [uv package manager](https://github.com/astral-sh/uv) and follows similar patterns for dependency group resolution.
