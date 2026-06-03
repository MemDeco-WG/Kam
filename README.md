# Kam - CLI toolkit for KernelSU, APatch, Magisk, and AnyKernel3 modules

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

English | [中文](README.zh-CN.md)

Kam is a CLI toolkit for scaffolding, building, packaging, installing, and
publishing Android root module projects. It supports KernelSU, APatch, Magisk,
and AnyKernel3 workflows, with templates for both full source repositories and
metadata-only module registry repositories.

Kam is under active development. Use the latest release or build from source
when testing new template, workflow, and device-development features.

## Supported Targets

- KernelSU modules, including KernelSU Modules Repo full-source and
  reference-only layouts.
- APatch modules with compatible install and runtime scripts.
- Magisk modules using the standard root-level `module.prop` layout.
- AnyKernel3 kernel ZIP projects through `ak3_template` / `-t ak3`.

The standard `kam_template` / `-t kam` template creates a Magisk-style module
ZIP that can work across Magisk-compatible managers, including KernelSU and
APatch, when the module scripts themselves are manager-compatible.

## Features

- Fast project initialization from built-in, local, archive, and cached
  templates.
- Release-oriented module ZIP builds.
- Development sessions with adb sync, hot update, logs, forwarding, and MCP.
- Metadata sync from `kam.toml` to `module.prop`, `update.json`, `module.json`,
  `repo.json`, `track.json`, and `config.json`.
- KernelSU developer key generation, submission, revocation, and certificate
  helpers.
- Template import, export, cache, pull, and update flows.
- GitHub Actions workflow installation for build/release or release mirroring.
- Project-local and global configuration via `kam config`.

## Installation

Recommended installer for Linux, macOS, Termux, and Windows Git Bash/MSYS:

```bash
git clone https://github.com/MemDeco-WG/Kam.git
cd Kam
./install.sh
```

Manual Cargo install:

```bash
cargo install kam
```

Build from source without installing:

```bash
git clone https://github.com/MemDeco-WG/Kam.git
cd Kam
cargo build --release
```

Platform notes:

- macOS requires `curl` and a C compiler. If Xcode Command Line Tools are
  missing, the installer starts `xcode-select --install`.
- Termux installs required packages with `pkg install -y curl git clang make
  pkg-config openssl perl`.
- Windows should use Git Bash or MSYS2. If native compilation fails, install
  the MSYS2 mingw-w64 toolchain or Visual Studio Build Tools.

## Quick Start

Create a standard KernelSU/APatch/Magisk-compatible module:

```bash
kam init my_module -t kam
cd my_module
kam build
```

Create an AnyKernel3 kernel module:

```bash
kam init my_kernel_module -t ak3
```

Create a KernelSU Modules Repo full-source repository:

```bash
kam init my_module \
  --repo-mode full \
  --source-url https://github.com/you/my_module \
  -t kam_template
```

Create a KernelSU Modules Repo reference-only repository:

```bash
kam init my_module \
  --repo-mode reference \
  --source-url https://github.com/you/my_module-source \
  --project-name "My Module" \
  --description "Short module summary"
```

For KernelSU Modules Repo compatibility, the repository name must match
`module.prop` `id`, releases must be immutable non-draft GitHub Releases, and
release ZIP assets must contain a root-level `module.prop` with `id`, `version`,
and `versionCode`.

## Templates

Built-in template aliases:

| Alias | Full template | Use case |
| --- | --- | --- |
| `-t kam` | `kam_template` | Standard user-space module |
| `-t meta` | `meta_template` | Metadata/metamodule workflows |
| `-t ak3` | `ak3_template` | AnyKernel3 kernel ZIP projects |
| `--tmpl` | `tmpl_template` | Template authoring project |

Examples:

```bash
kam tmpl list
kam tmpl pull
kam init my_module -t kam
kam init my_module -t ./tmpl/my_template
kam init my_module -t ./templates/my_template.tar.gz
kam init my_module -t kam --var repository=https://github.com/you/my_module
```

Template resolution order is direct local path, built-in template asset,
project-local `tmpl/` or `templates/`, then global template cache. If a simple
name does not end with `_template`, Kam also tries `<name>_template`.

See:

- [Template documentation index](docs/templates.md)
- [Template development specification](docs/template-development.md)
- [Kam TOML specification](docs/kam-toml.md)

## Common Commands

```bash
kam init my_module -t kam
kam add script service
kam add hook pre-build sync-version --order 20
kam add kamfw watchdog --phase service
kam add webui
kam check
kam build
kam install dist/my_module.zip
kam dev --watch --hot --mcp --logs
kam diff --device auto
kam sync
kam workflow install https://github.com/you/my_module
```

Pacman-style registry shortcuts:

```bash
kam -Sy           # refresh the local module package index
kam -Ss magic     # search the local synced module index
kam -Si module_id # show local package metadata
kam -Sl           # list packages in the local synced index
kam -S module_id  # download a module by id from local package metadata
kam -Syu module_id # refresh index, then download
```

Installed module query shortcuts:

```bash
kam -Q                 # list installed modules on the connected device
kam -Qs magic          # search installed module metadata
kam -Qi module_id      # show installed module.prop metadata
kam -Q --device serial # query a specific adb device
```

Full command documentation lives in [docs/commands.md](docs/commands.md).

`kam diff` compares the installed device module with the current source tree
and filters out binary payloads before running diff.

## Module Configuration

Edit `kam.toml`:

```toml
[prop]
id = "my_module"
name = "My Module"
version = "1.0.0"
versionCode = 1
author = "YourName"
description = "An Android root module"
updateJson = "https://example.com/update.json"

[mmrl.repo]
repository = "https://github.com/you/my_module"
changelog = "https://github.com/you/my_module/blob/main/CHANGELOG.md"
```

Or use `kam config` / `kam toml`:

```bash
kam config set prop.author "YourName"
kam config --global set prop.author "YourName"
kam toml get mmrl.repo.repository
kam toml set prop.version=1.2.3
```

## Development Sessions

`kam build` is for release packaging. `kam dev` is for fast real-device
iteration:

```bash
kam dev
kam dev --watch
kam dev --sync-only
kam dev --install
kam dev --logs
kam dev --mcp
kam dev doctor
```

`kam dev --mcp` uses the Kam Dev Runtime Contract v1:

- Module root: `/data/adb/modules/<module_id>`
- Standard CLI: `/data/adb/modules/<module_id>/cli`
- Commands: `cli mcp enable`, `cli mcp disable`, `cli mcp status`,
  `cli mcp status --json`
- Transport: Streamable HTTP
- Default endpoint: `http://127.0.0.1:8765/mcp`

See [docs/commands.md](docs/commands.md) for command options and
[docs/advanced-usage.md](docs/advanced-usage.md) for hook/runtime details.

## Agent Skills

Kam includes repo-local agent skills for module and template work.

Install the template development skill in another agent workspace:

```bash
npx skills add https://github.com/MemDeco-WG/Kam --path .agents/skills/kam-template-development
```

If your skills installer expects a skill name:

```bash
npx skills add https://github.com/MemDeco-WG/Kam --skill kam-template-development
```

## Project Structure

```text
my_module/
├── kam.toml
├── src/
│   └── my_module/
│       ├── module.prop
│       ├── customize.sh
│       ├── service.sh
│       └── system/
├── hooks/
│   ├── pre-build/
│   └── post-build/
├── webui/
├── dist/
├── update.json
└── README.md
```

## More Documentation

- [Commands reference](docs/commands.md)
- [Advanced usage](docs/advanced-usage.md)
- [Kam TOML specification](docs/kam-toml.md)
- [Template development specification](docs/template-development.md)
- [Template documentation index](docs/templates.md)
- [setup-kam workflow skill](.agents/skills/setup-kam-workflow/SKILL.md)
- [Kam module development skill](.agents/skills/magisk-module-development/SKILL.md)
- [kamfw framework skill](.agents/skills/kamfw-framework-usage/SKILL.md)

## Contributing

Issues and pull requests are welcome.

1. Fork this repository.
2. Create a feature branch.
3. Run formatting, checks, tests, and clippy.
4. Commit scoped changes.
5. Open a pull request.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE).

## Acknowledgments

- [Magisk](https://github.com/topjohnwu/Magisk)
- [KernelSU](https://github.com/tiann/KernelSU)
- [APatch](https://github.com/bmax121/APatch)
- [MMRL](https://github.com/MMRLApp/MMRL)

## Contact

- GitHub Issues: <https://github.com/MemDeco-WG/Kam/issues>
- Author: LightJunction
