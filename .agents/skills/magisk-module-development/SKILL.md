---
name: magisk-module-development
description: Use this skill when developing, refactoring, validating, packaging, or releasing Magisk/KernelSU/APatch modules with Kam. Applies when the user asks to create a module, edit kam.toml, manage module.prop/update.json metadata, write module scripts or hooks, use Kam templates, run kam build/check/export/sign/verify/install, or diagnose Kam-built Android module packages.
---

# Magisk Module Development With Kam

## Operating Mode

Work in the target module repository, not only in the Kam source tree. Prefer real `kam` commands over manual ZIP editing. Preserve user changes and inspect the current module layout before edits.

Use this skill for Magisk-compatible modules as well as KernelSU and APatch targets when Kam is the build tool.

## First Pass

1. Inspect the repository shape:
   - `kam.toml`
   - `src/`
   - `hooks/pre-build/`
   - `hooks/post-build/`
   - `module.prop`, `update.json`, or exported metadata files
2. Check the available Kam version and command surface:
   - `kam --version`
   - `kam --help`
   - `kam build --help`
   - `kam check --help`
3. If the repo uses a local Kam checkout, prefer the project binary:
   - `cargo run -- build`
   - `cargo run -- check`
   - `cargo run -- export`
   Otherwise use the installed `kam` CLI.

## Create A Module

For a standard Magisk-style module:

```bash
kam init my_module -t kam_template
cd my_module
kam check
kam build
```

For template-backed variants:

```bash
kam tmpl list
kam init my_module -t kam_template
kam init my_meta_module -t meta_template
kam init my_kernel_module -t ak3_template
```

Use `--interactive` when project metadata is unclear and the user has not supplied enough details.

## Module Layout Rules

Keep module runtime files under `src/<module_id>/` unless the existing module uses a different Kam-supported layout.

Expected module payload usually includes:

- `module.prop` generated from `kam.toml`
- optional scripts: `post-fs-data.sh`, `service.sh`, `uninstall.sh`, `customize.sh`
- optional `system/`, `system_ext/`, `product/`, `vendor/` overlays
- optional `webroot/` or manager UI assets if the module supports them

Do not hand-edit generated `module.prop` as the source of truth. Update `kam.toml`, then run:

```bash
kam export
kam check
```

## kam.toml Guidance

Treat `kam.toml` as the authoritative metadata/config file. Keep these fields coherent:

- module id: stable, lowercase-ish, no spaces
- name: human-readable module name
- version and versionCode: bump intentionally for releases
- author, description, license, homepage/repository
- root manager compatibility if the module is manager-specific
- output settings under the Kam build/package section

After editing metadata, verify generated files:

```bash
kam export
kam check
git diff -- module.prop update.json '**/module.prop'
```

## Hooks

Use hooks for build-time generation, not runtime side effects.

- `hooks/pre-build/`: generate files, sync metadata, build web assets, prepare payloads.
- `hooks/post-build/`: inspect or sign artifacts, copy release files, emit summaries.
- `hooks/dev-build/`: local development build work for `kam dev`.
- `hooks/dev-webui/`: incremental WebUI rebuilds used by `kam dev --watch`
  before syncing `webroot/**`.
- `hooks/dev-binary/`: incremental CLI/binary rebuilds used by
  `kam dev --watch` before syncing `.local/bin/**`.
- `hooks/dev-sync/`: post-sync actions after hot files are pushed to a device.
- `hooks/dev-install/`: post-install actions for `kam dev --install`.
- Hook order is controlled by numeric prefixes.
- Template-provided hooks use uppercase marker names such as `0.EXAMPLE.sh`.

Hook scripts should be deterministic and fail loudly:

```bash
#!/usr/bin/env bash
set -euo pipefail
```

Use Kam-provided environment variables where available, such as module root, project root, build output paths, and template variables. Inspect existing hooks before inventing new variable names.

For Android inner-loop development, keep release-only checks in
`hooks/pre-build/` and put fast iteration work in dev hooks. `kam dev --watch`
should be able to rebuild only WebUI assets or only module binaries without
rerunning the full release build path.

For `kam dev --logs`, expect Kam to print `.kam/dev/last-session.log`, common
manager install logs, configured device module logs, and recent `logcat` lines
filtered by module id. Prefer adding project-specific log paths under
`[dev].logs` instead of hard-coding module names in Kam. Dev hooks receive
`KAM_DEV_SESSION_LOG`; use it when a hook needs to persist detailed output for
the next `kam dev --logs` run.

## Build And Validate

Minimum validation for module work:

```bash
kam check
kam build
```

For releases:

```bash
kam build --release
kam verify dist/*.zip
```

If signing is configured:

```bash
kam build --release --sign
kam verify dist/*.zip
```

When changing templates or Kam itself, also validate the relevant template path:

```bash
kam init /tmp/kam-module-smoke -t kam_template --force
cd /tmp/kam-module-smoke
kam check
kam build
```

## ZIP Inspection

When diagnosing install failures, inspect the produced ZIP instead of guessing:

```bash
unzip -l dist/*.zip
unzip -p dist/*.zip module.prop
```

Confirm that:

- `module.prop` is at ZIP root.
- runtime scripts are executable where required.
- overlay paths match Android partition expectations.
- no source-only files, secrets, caches, or `.git` directories are packaged.

## Runtime Safety

For Magisk/KernelSU/APatch module scripts:

- Use POSIX shell unless the manager explicitly supports more.
- Avoid host-only Bashisms in runtime scripts.
- Prefer idempotent operations; module scripts may run repeatedly.
- Log enough to diagnose failures, but do not dump secrets.
- Do not assume writable system partitions; use overlay/module paths.
- Gate destructive actions behind clear file/path checks.

## Kam Dev Runtime MCP Best Practice

When a Kam module exposes MCP for `kam dev --mcp`, `kam mcp`, or agent-driven
module development, treat `cli mcp ...` as the standard integration surface.
Do not make Kam know project-specific binary names, process managers, or server
paths.

- Module root: `/data/adb/modules/<module_id>`.
- Standard CLI: `/data/adb/modules/<module_id>/cli`.
- Recommended implementation: `cli -> .local/bin/<module_id>-cli` or another
  project binary kept under `.local/bin/`.
- MCP transport: HTTP Streamable MCP, reported as `streamable-http`.
- MCP endpoint: `http://127.0.0.1:<port>/mcp`, with `8765` as the default
  device and local port unless the project declares otherwise.
- Device binding: prefer `127.0.0.1:<port>` on Android and expose it to the
  workstation through `adb forward`, not through a public device interface.
- Required CLI commands:
  - `cli mcp enable`
  - `cli mcp disable`
  - `cli mcp status`
  - `cli mcp status --json`
- Command behavior:
  - `enable` starts or enables the module MCP server and is idempotent.
  - `disable` stops or disables the server and is idempotent.
  - `status` prints a human-readable summary.
  - `status --json` prints only JSON to stdout so Kam and agents can parse it.

`cli mcp status --json` should return stable machine-readable fields:

```json
{
  "enabled": true,
  "running": true,
  "pid": 1234,
  "host": "127.0.0.1",
  "port": 8765,
  "endpoint": "/mcp",
  "transport": "streamable-http"
}
```

Keep these fields stable even if the module has richer internal state. Add
extra fields only when they are backward-compatible.

Project defaults belong in `kam.toml`:

```toml
[dev.mcp]
enabled = true
port = 8765
local_port = 8765
endpoint = "/mcp"
transport = "streamable-http"
```

Kam-side behavior:

- `kam mcp forward` maps `tcp:<local_port>` to `tcp:<port>` with adb.
- `kam mcp enable` calls `/data/adb/modules/<module_id>/cli mcp enable`.
- `kam mcp status --json` calls the module CLI and expects JSON only.
- `kam dev --mcp` behaves like `kam mcp forward`, `kam mcp enable`, then
  `kam mcp status --json`.

Do not implement MCP as stdio or SSE for Android module dev unless the user
explicitly asks for a non-standard transport. The Kam module convention is HTTP
Streamable MCP behind adb port forwarding.

## Common Fix Patterns

- Build fails because generated metadata is stale: edit `kam.toml`, run `kam export`, then `kam check`.
- Manager cannot see module: inspect ZIP root, `module.prop`, and output filename.
- Hook works locally but not CI: remove host-specific absolute paths, require tools in preflight, and use Kam environment variables.
- Template variable missing: check `[kam.tmpl.variables]`, `.kam/template-vars.env.init`, and init command `--var key=value`.
- Release artifact changed unexpectedly: compare `unzip -l` output before and after the change.

## Delivery Checklist

Before finishing:

- Run the smallest real `kam check` / `kam build` path that covers the change.
- Report exact commands run and whether they passed.
- If a command cannot run because Kam or platform tools are missing, state the missing dependency and the unverified risk.
- Keep changes scoped to module config, hooks, payload files, or Kam templates relevant to the request.
