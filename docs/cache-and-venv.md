# Kam Cache and Virtual Environment System

This document describes the cache and virtual environment features in Kam.

## Table of Contents

- [Cache System](#cache-system)
- [Virtual Environment System](#virtual-environment-system)
- [Commands](#commands)
- [Usage Examples](#usage-examples)

## Cache System

Kam uses a global cache system inspired by [uv-cache](https://github.com/astral-sh/uv/tree/main/crates/uv-cache) to store modules, binaries, and templates.

### Cache Location

The cache location is platform-specific:

- **Non-Android** (Linux, macOS, Windows): `~/.kam/`
- **Android**: `/data/adb/kam`

### Directory Structure

```text
~/.kam/ (or /data/adb/kam on Android)
├── bin/      # Executable binary files (provided by library modules)
├── lib/      # Library modules (extracted dependencies, not compressed)
├── log/      # Log files
└── profile/  # Template module archives (compressed .zip files)
```

### Cache Operations

The cache system supports the following operations:

- **Initialize**: Automatically creates cache directories when needed
- **Info**: Display cache statistics (size, file count, paths)
- **Clear**: Remove all cached data
- **Clear Directory**: Remove cached data from a specific directory

## Virtual Environment System

Kam provides virtual environments similar to Python's virtualenv, allowing isolated dependency management for projects.

### Environment Types

1. **Development Environment**: Includes dev dependencies
2. **Runtime Environment**: Production-ready, no dev dependencies

### Virtual Environment Structure

```text
.kam-venv/
├── bin/         # Symlinks to cached binaries
├── lib/         # Symlinks to cached libraries
├── activate     # Activation script (Unix)
├── activate.sh  # Activation script (Unix)
├── activate.ps1 # Activation script (PowerShell)
├── activate.bat # Activation script (Windows)
└── deactivate   # Deactivation script
```

### Features

- **PATH Management**: Automatically updates PATH to include venv binaries
- **Prompt Updates**: Visual indicator when venv is active
- **Platform Support**: Works on Unix, Windows, and PowerShell
- **Symlinks**: Uses symlinks on Unix for efficiency, copies on Windows
- **Easy Deactivation**: Simple `deactivate` command to exit

## Commands

### `kam cache`

Manage the global cache.

#### Subcommands

##### `kam cache info`

Show cache information and statistics.

```bash
kam cache info
```

Output:
```
Kam Cache Information

  Root: /home/user/.kam

Directories:
  bin: /home/user/.kam/bin
  lib: /home/user/.kam/lib
  log: /home/user/.kam/log
  profile: /home/user/.kam/profile

Statistics:
  Total Size: 150.45 MB
  File Count: 42
```

##### `kam cache clear`

Clear all cache with confirmation.

```bash
kam cache clear

# Skip confirmation
kam cache clear --yes
```

##### `kam cache clear-dir <dir>`

Clear a specific directory (bin, lib, log, or profile).

```bash
# Clear log directory
kam cache clear-dir log

# Skip confirmation
kam cache clear-dir log --yes
```

##### `kam cache path`

Show the cache root path.

```bash
kam cache path
```

### `kam sync`

Synchronize dependencies from `kam.toml`.

```bash
# Sync normal dependencies
kam sync

# Sync with dev dependencies
kam sync --dev

# Sync and create virtual environment
kam sync --venv

# Sync dev dependencies and create development venv
kam sync --dev --venv
```

#### What `kam sync` does:

1. Loads `kam.toml` configuration
2. Resolves dependency groups (handles `include:` references)
3. Downloads missing modules to cache
4. Creates symlinks from cache to project
5. Optionally creates virtual environment

### `kam build`

Package the module according to `kam.toml` configuration.

```bash
# Build with default settings
kam build

# Build to custom directory
kam build --output ./custom-dist

# Build specific project
kam build ./my-project
```

#### What `kam build` does:

1. Loads `kam.toml` configuration
2. Runs pre-build hook (if specified)
3. Packages source files from `src/<module-id>/`
4. Creates zip archive with:
   - `kam.toml`
   - Source files
   - `README.md` (if exists)
   - `LICENSE` (if exists)
5. Runs post-build hook (if specified)
6. Outputs to `dist/` directory (or custom path)

### `kam init`

Initialize a new Kam project (already implemented).

See existing documentation for details.

## Usage Examples

### Example 1: Basic Workflow

```bash
# Initialize a new project
kam init my-module --name "My Module" --author "Your Name"

# Navigate to project
cd my-module

# Add dependencies to kam.toml (manually edit file)

# Sync dependencies
kam sync

# Build the module
kam build
```

### Example 2: Development Workflow with Virtual Environment

```bash
# Initialize project
kam init my-dev-module

# Navigate to project
cd my-dev-module

# Edit kam.toml to add dependencies
# [kam.dependency]
# normal = [
#     { id = "core-lib", version = "1.0.0" }
# ]
# dev = [
#     { id = "include:normal" },
#     { id = "test-framework", version = "2.0.0" }
# ]

# Sync with dev dependencies and create venv
kam sync --dev --venv

# Activate the virtual environment
# On Unix:
source .kam-venv/activate

# On Windows:
.kam-venv\activate.bat

# On PowerShell:
.kam-venv\activate.ps1

# Your prompt now shows: (kam-venv) $

# Work on your project...

# Deactivate when done
deactivate
```

### Example 3: Cache Management

```bash
# Check cache size and location
kam cache info

# Clear log files
kam cache clear-dir log --yes

# Clear everything (with confirmation)
kam cache clear

# Get cache path for scripts
CACHE_PATH=$(kam cache path)
echo "Cache is at: $CACHE_PATH"
```

### Example 4: Building with Hooks

Configure build hooks in `kam.toml`:

```toml
[kam.build]
target_dir = "dist"
output_file = "my-module-v1.0.0.zip"
pre_build = "npm run test"
post_build = "echo Build complete!"
```

Then build:

```bash
kam build
```

Output:
```
Building module...

  • Module: my-module v1.0.0
  • Output: dist

Running pre-build hook...
npm run test
...

Packaging source files...
  + kam.toml
  + src/my-module/main.sh
  + src/my-module/util.sh
  + README.md
  + LICENSE

✓ Built: dist/my-module-v1.0.0.zip

Running post-build hook...
Build complete!
```

## Integration with Dependencies

### Dependency Resolution

Kam supports advanced dependency resolution with group includes:

```toml
[kam.dependency]
# Base dependencies
normal = [
    { id = "core-lib", version = "1.0.0" },
    { id = "utils", version = "2.0.0" }
]

# Dev dependencies include all normal dependencies
dev = [
    { id = "include:normal" },
    { id = "test-framework", version = "3.0.0" },
    { id = "debugger", version = "1.5.0" }
]
```

When you run `kam sync --dev`, it resolves to:
- core-lib@1.0.0
- utils@2.0.0
- test-framework@3.0.0
- debugger@1.5.0

### Caching Strategy

1. **First Time**: Downloads module and stores in cache
2. **Subsequent Times**: Creates symlink to cached version
3. **Updates**: Detects version changes and downloads if needed

## Platform-Specific Behavior

### Unix-like Systems (Linux, macOS)

- Uses **symlinks** for efficiency
- Cache location: `~/.kam/`
- Activation: `source .kam-venv/activate`
- Executable permissions set automatically

### Windows

- Uses **file copies** instead of symlinks
- Cache location: `%USERPROFILE%\.kam\`
- Activation: 
  - CMD: `.kam-venv\activate.bat`
  - PowerShell: `.kam-venv\activate.ps1`

### Android

- Special cache location: `/data/adb/kam`
- Requires root access
- Symlinks supported if filesystem allows

## Best Practices

1. **Use Virtual Environments**: Always use `--venv` for development projects
2. **Cache Management**: Periodically run `kam cache info` to monitor size
3. **Clear Logs**: Run `kam cache clear-dir log` to clean up old logs
4. **Dev Dependencies**: Use `--dev` only in development, not in production
5. **Build Hooks**: Use pre-build hooks for tests, post-build for notifications

## Troubleshooting

### Cache Permission Denied

**Problem**: Cannot write to cache directory

**Solution**: 
```bash
# Check cache location
kam cache path

# Fix permissions (Unix)
chmod -R u+w ~/.kam

# Or use a custom cache location
export KAM_CACHE_DIR=/custom/path
```

### Virtual Environment Not Activating

**Problem**: Activation script doesn't work

**Solution**:
```bash
# Make sure you're sourcing, not executing
source .kam-venv/activate  # Correct
./.kam-venv/activate        # Wrong

# Check permissions (Unix)
chmod +x .kam-venv/activate
```

### Symlink Errors on Windows

**Problem**: Symlinks not supported

**Solution**: Kam automatically falls back to copying files on Windows. No action needed.

## Future Enhancements

Planned features for future releases:

1. **Lock Files**: Reproducible builds with dependency locking
2. **Version Constraints**: Advanced version resolution (>=, <, etc.)
3. **Conditional Dependencies**: Platform-specific dependencies
4. **Remote Cache**: Shared cache across machines
5. **Offline Mode**: Work without internet connection

## See Also

- [Dependency Resolution](./dependency-resolution.md)
- [Module Configuration](../README.md)
- [uv Cache System](https://github.com/astral-sh/uv/tree/main/crates/uv-cache)
