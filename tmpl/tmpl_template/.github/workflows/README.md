# setup-kam — GitHub Action

[![Release](https://img.shields.io/github/v/tag/MemDeco-WG/setup-kam?label=release)](https://github.com/MemDeco-WG/setup-kam/releases) [![License](https://img.shields.io/github/license/MemDeco-WG/setup-kam.svg)](LICENSE)

English README — For the Chinese version, see: [README-CN.md](./README-CN.md)

Introduction
---
`setup-kam` is a GitHub Action to install `kam` (a Rust-based CLI tool) in CI environments. It supports optional template import and cache acceleration. This repository includes two workflow examples: `exec.yml` (run arbitrary `kam` commands) and `init.yml` (initialize a project and optionally import templates). The sections below describe the inputs and how to trigger these workflows.

kam overview
---
`kam` is a lightweight module management tool providing dependency resolution, build, and module management.

Usage: `kam <COMMAND>`

Commands:
- `init`      Initialize a new Kam project
- `build`     Build the module
- `version`   Manage module version
- `cache`     Manage local cache
- `tmpl`      Manage templates (import/export)
- `validate`  Validate kam.toml configuration
- `help`      Show help for a command

Workflow: exec.yml (Run arbitrary commands)
---
The `exec.yml` workflow is triggered via `workflow_dispatch` and has an optional input `build-command`, defaulting to `kam build -r`. The workflow performs the following steps:
1. Checkout the repository.
2. Install `kam` using `MemDeco-WG/setup-kam` (configureable with `with.github-token` and `with.enable-cache`).
3. Print `kam --version`.
4. Execute `github.event.inputs.build-command` in a shell.

Key input (brief):
- `workflow_dispatch` -> `inputs` -> `build-command`: the full command string to run, e.g., `kam build -r`, `kam build --release`, or `kam validate`.

How to provide `build-command`:
- Through the GitHub UI: open the "Run workflow" dialog and fill in `build-command` (e.g., `kam build -r`).
- Using the `gh` CLI or API:
  - Example: `gh workflow run exec.yml -f build-command="kam build -r"`

Workflow: init.yml (Initialize project / Import templates)
---
`init.yml` accepts three configurable `workflow_dispatch` inputs:
- `template-url`: optional; default points to a template archive in repository releases. If empty, template import will be skipped.
- `init-command`: initialization command to run; default is `kam init . -t kam -f`.
- `enable-cache`: whether to enable caching (`true` or `false`); default `true`.

This workflow checks out the repository, executes `setup-kam` (with `template-url` and `enable-cache`), and then runs `init-command`.

How to set `template-url` and `init-command`:
- `template-url`:
  - Can point to a compressed archive (zip/tgz) such as `https://example.com/template.zip` or a release asset, or a local unzipped directory path like `./template-folder`.
  - If providing a URL, please keep the filename and extension (e.g. `.zip` / `.tgz`). `kam tmpl import` may reject files without recognizable extensions.
- `init-command`:
  - Provide the full command string to be executed by the Action. The default is `kam init . -t kam_template -f`.


setup-kam Action inputs (summary)
---
Common `with` inputs (used in both workflows):
- `github-token`: default `${{ github.token }}`. Used to access private releases/assets or perform authenticated operations.
- `enable-cache`: string `"true"`/`"false"`, default `"true"`. When enabled, this caches `kam` and `cargo` installations with version awareness to speed up subsequent runs.
- `template-url`: used by the init workflow to point to a template archive or local directory (see details above).

Recommendations & notes
---
- Use `uses: MemDeco-WG/setup-kam@v1` (major version tag) to avoid locking to a specific patch version.
- If `template-url` points to a compressed archive, retain the filename and extension (like `.zip`/`.tgz`), otherwise `kam tmpl import` might reject it.
- When pipeline hooks call `gh` commands, the `GH_TOKEN` environment variable may be required.
- Default inputs in the example workflows help you get started quickly and are customizable.

Triggering workflows (quick)
---
- From GitHub UI: Actions → select `exec.yml` or `init.yml` → Run workflow → provide inputs and run.
- From `gh` CLI/API: use `gh workflow run` or the REST API with appropriate inputs.

License
---
MIT
