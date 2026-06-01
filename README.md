# Kam - A CLI toolkit for scaffolding, building, and distributing ksu/APU/Magisk/AnyTemplate modules

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

English | [中文](README.zh-CN.md)

! [WARNING ⚠]It's under development. You can follow the development progress by checking the asl module in the kernelsu module repository. Testing will begin with the asl module.

## 📖 Overview


CI checks:
- `.github/workflows/i18n-check.yml` runs the exporter and verifies localized `--help` outputs
  (e.g., `kam build --help`, `kam tmpl import --help`) under `KAM_UI_LANGUAGE=zh`/`en` to ensure
  that help text is translated and to catch regressions early.
- The same workflow also verifies the exported JSON files are committed to the repository,
  preventing drift between CLI TOML and WebUI data.

Workflow goals:
- Share CLI `cli.*` translations with the WebUI so there's a single source of truth.
- Provide a maintenance script to discover missing translation keys and generate skeletons.
- Use CI to detect accidental regressions in CLI help localization.

Contributions to extend translations and improve coverage are welcome. Use the skeleton script to discover gaps, add keys to `src/i18n/en.toml` (and provide translations in `src/i18n/zh.toml`), then run the exporter and commit results.

Kam is a CLI toolkit for scaffolding, building, packaging, and distributing Android module packages and templates (ksu/APU/Magisk/AnyTemplate). It focuses on rapid project initialization, reproducible builds, template management, and convenient repository/metadata export for module maintainers and distribution channels.

### ✨ Key Features

- 🚀 **Quick Initialization** - Rapidly create new module projects using various templates
- 🔧 **Automated Build** - One-click module ZIP packaging
 - 🔒 **Network optional** - Kam supports offline operation for most commands, but some features may rely on network services for additional capabilities (see below).
- 🎯 **Smart Sync** - Auto-sync `kam.toml` configuration to `module.prop` and `update.json`
 - ⚙️ **Config Management** - `kam config` to manage global (`~/.kam/config.toml`) and project-level (`./.kam/config.toml`) settings to avoid repetitive edits
 - 🗂️ **Repo & Metadata Export** - Export `kam.toml` into repo.json, module.json, track.json, config.json for marketplaces or registries
- 🪝 **Hook System** - Support custom script hooks before/after builds
- 📦 **Template Management** - Import, export, and share module templates
- 🌐 **WebUI Integration** - Built-in WebUI building and integration (note: Kam does not provide runtime module management)
- 🔄 **Version Management** - Automated version numbering and release

## 🚀 Quick Start

### Installation

Recommended installer for macOS, Termux on Android, and Windows Git Bash/MSYS:

```bash
git clone https://github.com/MemDeco-WG/Kam.git
cd Kam
./install.sh
```

The installer detects the current platform, checks required tools, installs a
minimal Rust toolchain with `rustup` when `cargo` is missing, makes sure
`~/.cargo/bin` is available for the current shell, installs Kam from this
working tree, and verifies `kam --version`.

Platform notes:

- **macOS**: requires `curl` and a C compiler. If Xcode Command Line Tools are
  missing, the installer starts `xcode-select --install` and asks you to rerun.
- **Android / Termux**: installs required packages with `pkg install -y curl git
  clang make pkg-config openssl perl`.
- **Windows**: run from Git Bash or MSYS2. The installer can install Rust with
  rustup; if native compilation fails, install the MSYS2 mingw-w64 toolchain or
  Visual Studio Build Tools.

Manual Cargo install is still supported:

```bash
cargo install kam
```

Or build from source without installing:

```bash
git clone https://github.com/MemDeco-WG/Kam.git
cd Kam
cargo build --release
```

<details>
<summary>Details</summary>

### Create a New Module

Using Kam template:

```bash
kam tmpl list

kam tmpl pull # download online

kam init my_awesome_module -t kam_template
```

Using Meta template (meta-module):

```bash
kam init my_meta_module -t meta_template
```

Using AnyKernel3 template (kernel module):

```bash
kam init my_kernel_module -t ak3_template
```

Pacman-style top-level flags
- `-Ss` — search the remote modules registry (example: `kam -Ss <term>`).
- `-S`  — download a module by ID (example: `kam -S <moduleId>`).
- `-u, --update` — refresh the modules index before downloading (equivalent to `kam repo sync --force`).
- You can combine flags, e.g. `kam -Syu` will update the index (`-u`), assume yes (`-y`), then attempt downloads (`-S`).


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

### KernelSU Developer Keys

Kam can prepare the KernelSU developer key lifecycle used by <https://developers.kernelsu.org/>.

```bash
kam secret ksu-generate --name kernelsu-developer
kam secret ksu-submit --username <github-user> --public-key kernelsu-developer.public.pem --open
kam secret ksu-revoke --username <github-user> --serial-number <serial> --reason lost --open
```

`ksu-generate` creates a P-256 key pair. When `gpg` is available in an interactive terminal, Kam stores the private key as `*.private.pem.gpg`; pass `--no-gpg` to force a 0600 PEM file.

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
| `-t kam_template` (`-t kam`) | Standard Kam module | General module development |
| `-t meta_template` (`-t meta`) | Meta-module template | Meta modules (modules of modules) |
| `-t ak3_template` (`-t ak3`) | AnyKernel3 template | Kernel modules |
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

 - Download templates from a remote URL (downloaded to a temp file and imported via `kam tmpl import -f`). The download link is recorded in the global config (`~/.kam/config.toml`) as `tmpl.pull.url`.
 - The last download timestamp is stored as `tmpl.pull.last_download`.

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

#### Additional Template Commands

```bash
# Remove a template from cache
kam tmpl remove template_name

# Show template cache directory
kam tmpl path
```

For the current configuration and template rules, see:

- [Kam TOML specification](docs/kam-toml.md)
- [Template development specification](docs/template-development.md)
- [Template documentation index](docs/templates.md)

## 📖 Commands Reference



### `kam init` - Initialize a New Project

Initialize a new Kam project from templates (supports meta and kernel templates).

```bash
kam init [OPTIONS] [PATH]
```

**Arguments:**
- `[PATH]` - Path to initialize the project. When running interactively (`-i`/`--interactive`), this PATH may be omitted; the interactive flow will prompt for it instead.

**Options:**
- `--id <ID>` - Project ID (default: folder name)
- `--project-name <PROJECT_NAME>` - Project name (default: "Example Module Name")
- `--version <VERSION>` - Project version (default: "1.0.0")
- `--author <AUTHOR>` - Author name (default: "Your Name")
- `--update-json <UPDATE_JSON>` - Update JSON URL (default: auto-generated from git)
- `--description <DESCRIPTION>` - Description (default: "Describe your module here")
- `-f, --force` - Force overwrite existing files
- `-i, --interactive` - Run the init interactively; ask for required values
- `--var <VAR>` - Template variables in key=value format
- `-t, --template <TEMPLATE>` - Template to use (built-in ID or local path)
- `--tmpl` - Create a template project (Template id: "tmpl_template")

**Examples:**
```bash
kam init my_module -t kam_template
kam init my_module -t meta_template --interactive
kam init my_module --tmpl
```

### `kam build` - Build and Package Module

Build and package a module into a deployable ZIP artifact.

```bash
kam build [OPTIONS] [PATH]
```

**Arguments:**
- `[PATH]` - Path to the project (default: current directory)

**Options:**
- `-a, --all` - Build all workspace members
- `-o, --output <OUTPUT>` - Output directory (default: dist)
- `-b, --bump` - Enable KAM_BUMP_ENABLED environment variable (set to 1)
- `-r, --release` - Enable KAM_RELEASE_ENABLED environment variable (set to 1)
- `-s, --sign` - Enable KAM_SIGN_ENABLE environment variable (set to 1)
- `-i, --interactive` - Run build interactively; ask for confirmation when performing potentially destructive actions
- `-P, --pre-release` - Enable KAM_PRE_RELEASE environment variable (set to 1)
- `-q, --quiet` - Suppress most output; show only warnings and errors

**Examples:**
```bash
kam build
kam build --all
kam build --bump
kam build --release --sign
kam build --interactive
```

### `kam version` - Manage Module Versions

Manage module versions and bump policies.

```bash
kam version [VERSION]
```

**Arguments:**
- `[VERSION]` - The new version (e.g. 1.0.1) or bump type (major, minor, patch)

**Examples:**
```bash
kam version 1.0.1
kam version patch
kam version minor
kam version major
```

### `kam tmpl` - Template Management

Manage templates: import, export, package, and list.

```bash
kam tmpl <COMMAND>
```

**Subcommands:**

#### `kam tmpl list` - List Available Templates
```bash
kam tmpl list
```

#### `kam tmpl import` - Import Template(s)
```bash
kam tmpl import [OPTIONS] <PATH>
```
- `<PATH>` - Path to template archive (.tar.gz for single template, .zip for multiple templates)
- `-n, --name <NAME>` - Template name (optional, will use filename if not provided)
- `-f, --force` - Force overwrite if template already exists

#### `kam tmpl export` - Export Template(s)
```bash
kam tmpl export [OPTIONS] --output <OUTPUT> [TEMPLATES]...
```
- `[TEMPLATES]...` - Template name(s) to export (can specify multiple)
- `-o, --output <OUTPUT>` - Output file path (.tar.gz for single template, .zip for multiple templates)
- `-f, --force` - Force overwrite if output file already exists

#### `kam tmpl pull` - Download Templates
```bash
kam tmpl pull [OPTIONS] [URL]
```
- `[URL]` - Download URL (defaults to GitHub latest release templates ZIP)
- `--global` - (NOTE: URLs are always recorded in global config: `~/.kam/config.toml`) The `--global` flag is accepted for CLI consistency but has no effect

#### `kam tmpl update` - Update Templates
Re-download based on recorded URL in config and import.

```bash
kam tmpl update
```

#### `kam tmpl remove` - Remove Template
```bash
kam tmpl remove <TEMPLATE>
```

#### `kam tmpl path` - Show Template Cache Directory
```bash
kam tmpl path
```

### `kam cache` - Manage Local Cache

Manage local template and artifact cache.

```bash
kam cache <COMMAND>
```

**Subcommands:**
- `kam cache list` - List cached templates
- `kam cache clean` - Clean all cached templates
- `kam cache add` - Add a template to cache from a local directory or archive
- `kam cache remove` - Remove a template from cache
- `kam cache path` - Show cache directory path

### `kam validate` - Validate Configuration

Validate `kam.toml` configuration and templates.

```bash
kam validate [PATH]
```

**Arguments:**
- `[PATH]` - Path to the project directory (default: current directory)

### `kam check` - Check Project Files

Check project JSON/YAML/Markdown files (lint/format/parse).

```bash
kam check [OPTIONS] [PATH]
```

**Arguments:**
- `[PATH]` - Path to the project directory (default: current directory)

**Options:**
- `--json` - Output results as JSON

**Note:** `kam check` requires `shellcheck` to be installed on your PATH to check shell scripts. If `shellcheck` is not installed, `kam check` will fail with the message: 请安装shellcheck
- `--fix` - Try to automatically fix/format files

**Examples:**
```bash
kam check
kam check --json
kam check --fix
```

### `kam install` - Install a module package to a connected device

Install a built module (.zip) to a connected device using the configured root manager. Kam will attempt to run the preferred install CLI (e.g., `magisk`, `ksud`, `apd`) and, if necessary, will attempt privilege escalation via `su -c`. When escalation is used, output is streamed live and any interactive prompts are forwarded to your terminal.

```bash
kam install [OPTIONS] [PATH]
```

**Arguments:**
- `[PATH]` - Path to the module package (.zip). If omitted, Kam will attempt to find the artifact in the project's `dist/` output directory.

**Options:**
- `--manager <Manager>` - Preferred root manager to use (Magisk, KernelSU, APatchSU). Overrides configured default.
- `--dry-run` - Print the derived install command without executing it.
- `-q, --quiet` - Suppress non-essential output.

**Examples:**
```bash
kam install my_module.zip
kam install --manager KernelSU my_module.zip
kam install --dry-run my_module.zip
```

### `kam export` - Export Configuration

Export `kam.toml` to `module.prop`, `module.json`, `repo.json`, `track.json`, `config.json`, `update.json`.

```bash
kam export [FORMAT] [OUTPUT]
```

**Arguments:**
- `[FORMAT]` - Export format: prop, json, update, repo, track, config
- `[OUTPUT]` - Output file path (default: write to a format-specific filename in the current directory)

**Examples:**
```bash
kam export prop
kam export json module.json
kam export update
kam export repo
```

### `kam toml` - TOML Manipulation

Inspect and edit `kam.toml` using dot-path keys (get/set/unset/list).

```bash
kam toml [OPTIONS] <COMMAND>
```

**Options:**
- `--file <FILE>` - Operate on the project's kam.toml (default), or specify file using --file

**Subcommands:**
- `kam toml get <KEY>` - Get a value by dot-separated key path
- `kam toml set <KEY> <VALUE>` - Set a value by key (usage: `kam toml set prop.name=value` or `kam toml set prop.name value`)
- `kam toml unset <KEY>` - Unset/remove a key
- `kam toml list` - Dump the full toml

**Examples:**
```bash
kam toml get mmrl.repo.repository
kam toml set prop.name "My Module"
kam toml set prop.version=1.2.3
kam toml unset prop.not_used
kam toml list
```

### `kam config` - Configuration Management

Manage per-project or global kam configuration (similar to git config).

```bash
kam config [OPTIONS] <COMMAND>
```

**Options:**
- `--global` - Use the global configuration file (`~/.kam/config.toml`)
- `--local` - Force use of the local configuration file (project `.kam/config.toml`)

**Subcommands:**
- `kam config get <KEY>` - Get a configuration value by key (dot-separated path)
- `kam config set <KEY> <VALUE>` - Set a configuration value by key
- `kam config unset <KEY>` - Unset (remove) a configuration value by key
- `kam config list` - List all config values in the target file

**Examples:**
```bash
kam config set prop.author "YourName"
kam config --global set prop.author "YourName"
kam config get prop.author
kam config list
```

### `kam secret` - Secret Keyring Management

Secret keyring management (used by sign/verify tasks).

```bash
kam secret <COMMAND>
```

**Subcommands:**
- `kam secret list` - List saved secrets
- `kam secret add <NAME> [FILE]` - Add a secret from a value or file
  - `-f, --file <FILE>` - Path to a file to read the secret from
  - `-v, --value <VALUE>` - Provide value directly
  - `--force-file` - Force storing to local file instead of system keyring
  - `--password <PASSWORD>` - Pass the password on the CLI (not recommended); password will be prompted if not set
  - `--with-backup` - Also create a local fallback file under ~/.kam/secrets
- `kam secret get <NAME>` - Get a secret and print it to stdout (or --out file)
- `kam secret remove <NAME>` - Remove a secret
- `kam secret export <NAME>` - Export secret to a file (by default decrypted). Use --encrypted to export encrypted blob
- `kam secret import <NAME> <FILE>` - Import secret from a file. If file is an encrypted KAM blob, it will be stored as-is
- `kam secret export-pub <NAME>` - Export public key from a stored private key secret
- `kam secret import-cert` - Import developer certificate chain from GitHub issue or file
- `kam secret trust` - Manage trusted Root CAs

**Examples:**
```bash
kam secret add main --file private_key.pem
kam secret list
kam secret export-pub main
```

### `kam sign` - Sign Artifacts

Sign an artifact using a key from the keyring or a PEM file.

```bash
kam sign [OPTIONS] [SRC]
```

**Arguments:**
- `[SRC]` - The artifact to sign (zip). If omitted, use --dist or --all to sign multiple files

**Options:**
- `--secret <SECRET>` - Name of the secret in kam keyring that holds the private key [default: main]
- `--out <OUT>` - Output directory (default: dist)
- `--dist <DIR>` - Sign all artifacts in given directory (instead of specifying single src file)
- `--all` - Sign all artifacts inside dist (alias of --dist <dir> with default dist)
- `--cert <CERT>` - Certificate PEM chain path to include in signature metadata
- `--key-path <KEY_PATH>` - Optional path to a private key PEM file to use instead of the keyring secret

**Examples:**
```bash
kam sign module.zip
kam sign --all
kam sign --dist dist --cert cert.pem
```

### `kam verify` - Verify Signatures

Verify an artifact signature (.sig) or a sigstore bundle (DSSE).

```bash
kam verify [OPTIONS] [SRC]
```

**Arguments:**
- `[SRC]` - Path to the artifact to verify (required for .sig verification)

**Options:**
- `--sig <SIG>` - Path to signature file (base64 .sig). If omitted, defaults to <src>.sig
- `--bundle <BUNDLE>` - Path to .sigstore.json bundle containing DSSE envelope and certs
- `--cert <CERT>` - Optional certificate PEM to use for verification (overrides bundle cert)
- `--root <ROOT>` - Optional root CA PEM to verify a certificate chain (trusted anchor)
- `--secret <SECRET>` - Name of secret in kam keyring that holds the private key; used to derive public key for verification [default: main]
- `--key <KEY>` - Path to public key PEM for verification (overrides derived key from secret)
- `--cert-name <CERT_NAME>` - Name of cached developer certificate to use for verification
- `--cert-chain <CERT_CHAIN>` - Path to certificate chain PEM file for verification
- `--skip-crl` - Skip CRL (Certificate Revocation List) check
- `-v, --verbose` - Verbose output showing verification steps

**Examples:**
```bash
kam verify module.zip
kam verify module.zip --sig module.zip.sig
kam verify module.zip --bundle module.zip.sigstore.json
kam verify module.zip --cert cert.pem --root root.pem
```

### `kam completions` - Generate Shell Completions

Generate shell completion scripts for common shells.

```bash
kam completions [OPTIONS] <SHELL>
```

**Arguments:**
- `<SHELL>` - Shell type for completion (bash, zsh, fish, powershell, elvish)

**Options:**
- `-o, --out <OUT>` - Output file. If omitted, prints to STDOUT
- `--install` - Install the completion script into the shell's completion directory (may require root)

**Examples:**
```bash
kam completions bash > /etc/bash_completion.d/kam
kam completions fish -o ~/.config/fish/completions/kam.fish
kam completions zsh --install
```

### `kam about` - Display About Information

Display about information for Kam and credits.

```bash
kam about
```

### ⚠️ Network & Optional Online Features

Kam supports optional network-backed functionality to increase security and convenience. These features are not required for basic scaffolding and builds, but may be enabled by flags or in future updates:

- **Timestamped signatures / Sigstore** — Using `kam sign` with Sigstore/timestamping enabled may contact a timestamp authority (TSA) or Sigstore services to generate RFC 3161 timestamped signatures or to record signatures on transparency logs (Rekor). This requires network access when enabled.
  Note: `kam sign` does not request an RFC3161 timestamp by default. Use `--timestamp` to enable timestamping when needed.
- **Template downloads (planned)** — A `kam tmpl pull` command will be added to make it easy to fetch and import templates from remote repositories or template registries.

If you're unsure how to run a particular `kam` command or want interactive help, you can run the Kamcp (MCP) server which exposes a `kam_exec` tool and an AI assistant that can explain commands or run them for you. See https://github.com/MemDeco-WG/Kamcp for installation and usage instructions. Example interactions include asking the AI "How do I use `kam tmpl pull`?" or using `kam_exec("-Ss <term>")` to search the modules registry.

When possible, these features are optional and disabled by default.

### Build Options
### TOML Manipulation
You can inspect and modify your `kam.toml` directly from the CLI using the `toml` subcommand:

```bash
# Get a nested value by dot-separated key path
kam toml get mmrl.repo.repository

# Set a value: support both `key value` and `key=value` forms
kam toml set prop.name "My Module"
kam toml set prop.version=1.2.3

# Remove a value
kam toml unset prop.not_used

# Dump kam.toml
kam toml list
```


```bash
# Basic build
kam build

# build all
kam build -a    # shorthand for --all
kam build --all

# Build with automatic version bump
kam build --bump

# Build and create GitHub Release
# Creates a GitHub release and uploads artifacts from `dist/` (signing and immutability optional)
kam build --release
#
# Example: Create an immutable signed release (skip re-upload if the same tag exists) and upload Sigstore
# attestation JSON (DSSE bundle copied as `*.attestation.json`) as release assets:
#
# kam build -r -s -i

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
├── 0.EXAMPLE.sh               # Example post-build hook (template)
├── 1.VERIFY.sh                # Verify build
├── 2.UPLOAD.sh                # Upload artifacts
└── 3.NOTIFY.sh                # Send notifications
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
| `KAM_HOME` | Override Kam's home directory (defaults to `~/.kam/`). This controls where global configuration, caches, and secrets are stored. |

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
# Common exclude patterns (these are written to generated `kam.toml` so users see and can edit them)
exclude = [
    ".git/",
    "target/",
    "node_modules/",
    ".DS_Store",
    "Thumbs.db",
    "*.tmp",
    "*.log",
    "*.bak",
    ".kam/",
]
# Explicit include patterns (defaults to an empty array). When a path matches both exclude and include,
# include takes precedence and the file will be packaged.
include = []
# Whether to respect a top-level .gitignore file. Default is false; prefer explicit exclude/include rules
# in `kam.toml` to control packaging behavior.
respect_gitignore = false
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

</details>

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
