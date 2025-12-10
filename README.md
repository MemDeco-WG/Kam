# Kam - Offline-first module scaffolding, packaging, and template toolkit

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Version](https://img.shields.io/badge/version-0.3.0-blue.svg)](https://github.com/MemDeco-WG/Kam)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

English | [中文](README.zh-CN.md)

## 📖 Overview

Kam is an offline-first CLI toolkit for scaffolding, building, and distributing Android module packages and templates. It focuses on rapid project initialization, reproducible offline builds, template management, and convenient repository/metadata export for module maintainers and distribution channels. Kam still supports building modules for Magisk, KernelSU, and APatch workflows.



### ✨ Key Features

- 🚀 **Quick Initialization** - Rapidly create new module projects using various templates
- 🔧 **Automated Build** - One-click module ZIP packaging
 - 🔒 **Offline-first (network optional)** - Kam is designed to work offline and does not require network access for most commands. However, some commands optionally rely on network services for additional capabilities (see below).
- 🎯 **Smart Sync** - Auto-sync `kam.toml` configuration to `module.prop` and `update.json`
 - ⚙️ **Config Management** - `kam config` to manage global (`~/.kam/config.toml`) and project-level (`./.kam/config.toml`) settings to avoid repetitive edits
 - 🗂️ **Repo & Metadata Export** - Export `kam.toml` into repo.json, module.json, track.json, config.json for marketplaces or registries
- 🪝 **Hook System** - Support custom script hooks before/after builds
- 📦 **Template Management** - Import, export, and share module templates
- 🌐 **WebUI Integration** - Built-in WebUI building and integration (note: Kam does not provide runtime module management)
- 🔄 **Version Management** - Automated version numbering and release

## 🚀 Quick Start

### Installation

```bash
cargo install kam
```

Or build from source:

```bash
git clone https://github.com/MemDeco-WG/Kam.git
cd Kam
cargo build --release
```

### Create a New Module

Using Kam template:

```bash
kam init my_awesome_module -t kam
```

Using Meta template (meta-module):

```bash
kam init my_meta_module -t meta
```

Using AnyKernel3 template (kernel module):

```bash
kam init my_kernel_module -t ak3
```

Using Astrbot template.

```bash
kam init my_astrbot_module -t astr_template
```

### Configure Your Module

Edit the `kam.toml` configuration file:

```toml
[prop]
id = "my_awesome_module"
name = "My Awesome Module"
version = "1.0.0"
versionCode = 1
author = "YourName"
description = "An awesome module for Android"
updateJson = "https://example.com/update.json"

[mmrl.repo]
repository = "https://github.com/username/my_awesome_module"
changelog = "https://github.com/username/my_awesome_module/blob/main/CHANGELOG.md"
```

### Manage Kam Configuration

Kam provides a `kam config` command to manage per-project and global configuration, similar to `git config`:

Examples:

```bash
# Set a project-level configuration (stored in `./.kam/config.toml`)
kam config set prop.author "YourName"

# Get a project-level configuration
kam config get prop.author

# Set a global configuration (stored in `~/.kam/config.toml`)
kam config --global set prop.author "YourName"

# List configuration of current target (project or global)
kam config list
```

This avoids frequent manual edits of `kam.toml` for values that should be global or common across projects.

### Add Module Files

Add your module files to the `src/<module_id>/` directory:

```
src/my_awesome_module/
├── module.prop          # Auto-generated
├── customize.sh         # Installation script
├── service.sh           # Service script
├── system/              # System files
│   └── bin/
│       └── my_script
└── webroot/             # WebUI files (optional)
```

### Build Your Module

```bash
kam build
```

The built module will be generated in the `dist/` directory.

## 📚 Documentation

### Template Types

Kam provides several built-in templates:

| Template | Description | Use Case |
|----------|-------------|----------|
| `-t kam` | Standard Kam module (maps to `kam_template`) | General module development |
| `-t meta` | Meta-module template (maps to `meta_template`) | Meta modules (modules of modules) |
| `-t ak3` | AnyKernel3 template (maps to `ak3_template`) | Kernel modules |
| `--tmpl` | Template development template (maps to `tmpl_template`) | Creating new templates |

### Template Management

#### Import Templates

Import a single template:
```bash
kam tmpl import templates/meta_template.tar.gz
```

Import multiple templates from a ZIP file:
```bash
kam tmpl import templates.zip
```

#### List Available Templates

```bash
kam tmpl list
```

#### Export Templates

Export a single template:
```bash
kam tmpl export meta_template -o my_template.tar.gz
```

Note: When exporting templates as a single `.tar.gz` (template packaging), Kam will not execute pre-build or post-build hooks. Template packaging is treated as an artifact operation and hooks are not applied.

Export multiple templates to a ZIP:
```bash
kam tmpl export kam_template ak3_template -o my_templates.zip
```

#### Download / Update Templates (new)

- Pull templates from a remote URL and import them into the local cache (downloads to a temp file and imports using `kam tmpl import -f`). The download link is recorded in the global config (~/.kam/config.toml).
 - Pull templates from a remote URL and import them into the local cache (downloads to a temp file and imports using `kam tmpl import -f`). The download link is recorded in the global config (~/.kam/config.toml) as `tmpl.pull.url`.
  The last download timestamp is stored as `tmpl.pull.last_download`.

```bash
# Use default download link (recorded in global config)
kam tmpl pull

# Provide URL (the URL will be recorded in global config)
kam tmpl pull https://example.com/templates.zip
```

- Update (re-download) using the recorded link saved by `kam tmpl pull`:

```bash
kam tmpl update
```
```

#### Additional Template Commands

```bash
# Remove a template from cache
kam tmpl remove template_name

# Show template cache directory
kam tmpl path
```

For more details on templates, see [templates/README.md](templates/README.md).

### ⚠️ Network & Optional Online Features

Kam is offline-first, but supports optional network-backed functionality to increase security and convenience. These features are not required for basic scaffolding and builds, but may be enabled by flags or in future updates:

- **Timestamped signatures / Sigstore** — Using `kam sign` with Sigstore/timestamping enabled may contact a timestamp authority (TSA) or Sigstore services to generate RFC 3161 timestamped signatures or to record signatures on transparency logs (Rekor). This requires network access when enabled.
- **Template downloads (planned)** — A `kam tmpl pull` command will be added to make it easy to fetch and import templates from remote repositories or template registries.

When possible, these features are optional and disabled by default to preserve the offline-first behavior of Kam.

### Build Options

```bash
# Basic build
kam build

# build all
kam build -a    # shorthand for --all
kam build --all

# Build with automatic version bump
kam build --bump

# Build and create GitHub Release
kam build --release

# Debug mode
KAM_DEBUG=1 kam build
```

### Check Project Files

Verify common data files in the project (JSON, YAML, Markdown). This command checks for parse errors and basic formatting issues; add `--fix` to attempt automatic fixes.

```bash
# Check current directory and print results (human friendly)
kam check

# Output results as JSON
kam check --json

# Attempt to auto-fix/format files
kam check --fix
```

### Hook System

Kam supports executing custom scripts during the build process:

Note: The hook runner executes files directly and does not perform OS-specific interpreter selection or special-case file extensions. It simply executes each file found in the hooks directory and defers to the operating system for execution. Ensure your hook scripts are runnable on your target environment (for example, include a shebang and mark the script executable on Unix with `chmod +x`, or run shell scripts via WSL/Git Bash on Windows).

#### Pre-build Hooks

Create scripts in the `hooks/pre-build/` directory:

- examples

```bash
hooks/pre-build/
├── 0.EXAMPLE.sh              # Example pre-build hook (template)
├── 1.SYNC_MODULE_FILES.sh    # Sync configuration files (script)
├── 2.BUILD_WEBUI.sh          # Build WebUI

```

#### Post-build Hooks

Create scripts in the `hooks/post-build/` directory:

```bash
hooks/post-build/
├── 0.verify.sh                # Verify build
├── 1.upload.sh                # Upload artifacts
└── 2.notify.sh                # Send notifications
```

#### Available Environment Variables

The following environment variables are available in hook scripts:

| Variable | Description |
|----------|-------------|
| `KAM_PROJECT_ROOT` | Absolute path to the project root directory |
| `KAM_HOOKS_ROOT` | Absolute path to the hooks directory |
| `KAM_MODULE_ROOT` | Absolute path to the module source directory (e.g., `src/<id>`) |
| `KAM_WEB_ROOT` | Absolute path to the module webroot directory |
| `KAM_DIST_DIR` | Absolute path to the build output directory (e.g., `dist`) |
| `KAM_MODULE_ID` | The module ID |
| `KAM_MODULE_VERSION` | The module version |
| `KAM_MODULE_VERSION_CODE` | The module version code |
| `KAM_MODULE_NAME` | The module name |
| `KAM_MODULE_AUTHOR` | The module author |
| `KAM_MODULE_DESCRIPTION` | The module description |
| `KAM_MODULE_UPDATE_JSON` | The module updateJson URL |
| `KAM_STAGE` | Current build stage: `pre-build` or `post-build` |
| `KAM_DEBUG` | Set to `1` to enable debug output |

### Auto-Sync

Kam automatically syncs `kam.toml` configuration to module files:

- **module.prop** → `$KAM_MODULE_ROOT/module.prop`
  - Contains module metadata (id, name, version, etc.)

- **update.json** → `$KAM_PROJECT_ROOT/update.json`
  - Contains update information (version, versionCode, zipUrl, changelog)
  - URLs are automatically inferred from `[mmrl.repo]` section

### WebUI Integration

Kam supports adding WebUI interfaces to modules:

1. Develop your frontend application in the `webui/` directory
2. WebUI will be automatically built and installed to `src/<module_id>/webroot/`
3. Access via the manager's WebUI feature after module installation

## 🔧 Advanced Usage

### Workspace

Kam supports workspace mode to manage multiple modules in one project:

```toml
[kam.workspace]
members = [
    ".",
    "modules/module_a",
    "modules/module_b",
]

# kam build --all
# equal to:
# kam build .
# kam build modules/module_a
# ...

```

### Custom Build Configuration

```toml
[kam.build]
target_dir = "dist"              # Output directory
output_file = "{{id}}-{{versionCode}}-{{version}}"           # Output filename template
hooks_dir = "hooks"              # Hooks directory
source_dir = "src/{{id}}"        # Source directory (optional)
```

### Conditional Compilation

Use template variables for conditional compilation:

```toml
[kam.tmpl.variables.feature_x]
var_type = "bool"
required = false
default = false
```

Use in scripts:

```bash
{% if feature_x %}
# Feature X related code
{% endif %}
```

## 📋 Project Structure

```
my_module/
├── kam.toml                    # Kam configuration file
├── src/
│   └── my_module/              # Module source code
│       ├── module.prop         # Module properties (auto-generated)
│       ├── customize.sh        # Installation script
│       ├── service.sh          # Service script
│       └── system/             # System files
├── hooks/
│   ├── pre-build/              # Pre-build hooks
│   └── post-build/             # Post-build hooks
├── webui/                      # WebUI source code (optional)
├── dist/                       # Build output
├── update.json                 # Update information (auto-generated)
└── README.md
```

## 🤝 Contributing

Contributions, issues, and feature requests are welcome!

1. Fork this repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [Magisk](https://github.com/topjohnwu/Magisk) - The Magic Mask for Android
- [KernelSU](https://github.com/tiann/KernelSU) - A Kernel-based root solution
- [APatch](https://github.com/bmax121/APatch) - Another kernel-based root solution

-[Mmrl](https://github.com/MMRLApp/MMRL) - Module repo.



## 📞 Contact

- GitHub Issues: [https://github.com/MemDeco-WG/Kam/issues](https://github.com/MemDeco-WG/Kam/issues)
- Author: LightJunction

---

Built with ❤️ and Rust
