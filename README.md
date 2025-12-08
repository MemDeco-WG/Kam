# Kam - KSU/APatch/Magisk Module Builder

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Version](https://img.shields.io/badge/version-0.3.0-blue.svg)](https://github.com/MemDeco-WG/Kam)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

English | [简体中文](README.zh-CN.md)

## 📖 Overview

Kam is a lightweight, networkless CLI tool for Magisk, KernelSU, and APatch module developers. It focuses on fast, offline-first module initialization (scaffolding) and packaging (build). Kam is designed to be minimal and work without network connectivity.

### Migration Note

The `[kam].dependency` section and related dependency management functionality have been removed. If your `kam.toml` contains a `dependency` section, please remove it before upgrading. Kam no longer resolves or manages external dependencies; use external tools or prebuild hooks to include required artifacts in the module source.


### ✨ Key Features

- 🚀 **Quick Initialization** - Rapidly create new module projects using various templates
- 🔧 **Automated Build** - One-click module ZIP packaging
- 🔒 **Lightweight & Offline** - Kam does not require network access and is designed to be minimal
- 🎯 **Smart Sync** - Auto-sync `kam.toml` configuration to `module.prop` and `update.json`
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

#### Additional Template Commands

```bash
# Remove a template from cache
kam tmpl remove template_name

# Show template cache directory
kam tmpl path
```

For more details on templates, see [templates/README.md](templates/README.md).

### Build Options

```bash
# Basic build
kam build

# build all
kam build --all

# Build with automatic version bump
kam build --bump

# Build and create GitHub Release
kam build --release

# Debug mode
KAM_DEBUG=1 kam build
```

### Hook System

Kam supports executing custom scripts during the build process:

#### Pre-build Hooks

Create scripts in the `hooks/pre-build/` directory:

- examples

```bash
hooks/pre-build/
├── 0.sync-module-files.sh    # Sync configuration files
├── 1.custom-script.sh         # Custom script
└── 2.another-script.sh
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
output_file = "{{id}}"           # Output filename template
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
