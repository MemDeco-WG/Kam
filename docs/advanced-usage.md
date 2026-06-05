# Kam Advanced Usage

This page covers build-time hooks, development hooks, workspace builds,
configuration sync, WebUI packaging, and optional online features.

## Release Hooks

Kam executes build hooks directly and lets the operating system choose the
interpreter. Hook scripts should include a shebang and executable bit.

Templates may restore official base hooks into `.kam/bases/hooks`. Treat that
directory as read-only because `kam sync bases --remote` updates it from the
remote base repository. Control official hooks from `.kam/bases.toml`: remove a
path from the `hooks` base `include` list to skip it, set `include = []` to
disable all official hooks, or add a same-name file under `hooks/<stage>/` to
override one hook locally. Extra files in `hooks/` are always user-owned.

Pre-build hooks live in:

```text
hooks/pre-build/
├── 0.EXAMPLE.sh
├── 1.SYNC_MODULE_FILES.sh
└── 2.BUILD_WEBUI.sh
```

Post-build hooks live in:

```text
hooks/post-build/
├── 0.EXAMPLE.sh
├── 1.VERIFY.sh
├── 2.UPLOAD.sh
└── 3.NOTIFY.sh
```

## Development Hooks

`kam dev` uses separate hooks from release builds:

```text
hooks/dev-build/
hooks/dev-webui/
hooks/dev-binary/
hooks/dev-sync/
hooks/dev-install/
hooks/dev-start/
hooks/dev-stop/
```

`kam dev --watch` runs narrower hooks when possible:

- `webui/**` or module `webroot/**` changes run `hooks/dev-webui/`, then sync
  `webroot/**`.
- `crates/**` or module `.local/bin/**` changes run `hooks/dev-binary/`, then
  sync `.local/bin/**`.
- allowlisted module scripts, templates, and property files are pushed directly
  with backup and rollback.
- structural changes are not hot-pushed automatically; run `kam dev --install`.

## Hook Environment

Hook scripts receive these common variables:

| Variable | Description |
| --- | --- |
| `KAM_PROJECT_ROOT` | Absolute project root |
| `KAM_HOOKS_ROOT` | Absolute hooks directory |
| `KAM_MODULE_ROOT` | Module source directory |
| `KAM_WEB_ROOT` | Module webroot directory |
| `KAM_DIST_DIR` | Build output directory |
| `KAM_MODULE_ID` | Module id |
| `KAM_MODULE_VERSION` | Module version |
| `KAM_MODULE_VERSION_CODE` | Module version code |
| `KAM_MODULE_NAME` | Module name |
| `KAM_MODULE_AUTHOR` | Module author |
| `KAM_MODULE_DESCRIPTION` | Module description |
| `KAM_MODULE_UPDATE_JSON` | Update JSON URL |
| `KAM_STAGE` | Current stage, such as `pre-build` or `dev-webui` |
| `KAM_DEV_SESSION_LOG` | Latest dev-session log for `dev-*` hooks |
| `KAM_DEBUG` | Set to `1` for debug output |
| `KAM_HOME` | Override global Kam home, defaulting to `~/.kam/` |

## Dev Configuration

Configure hot sync, logs, forwarding, and MCP in `kam.toml`:

```toml
[dev]
device = "auto"
module_path = "/data/adb/modules/MagicNet"
hot = ["webroot/**", "service.sh", "action.sh", ".local/bin/**", "templates/**"]
watch = [
  "webui",
  "crates",
  "src/MagicNet",
  "hooks/dev-build",
  "hooks/dev-webui",
  "hooks/dev-binary",
  "hooks/dev-sync",
]
logs = ["/data/adb/modules/MagicNet/logs/*.log"]
forward = ["mcp"]
webui_port = 8080
webui_local_port = 8080

[dev.sync]
stage_dir = "/sdcard/Download/kam-dev/{{id}}"
mirror = ["webroot/**"]
preserve = [".config/**", "config.yaml", "config.yml", "subscriptions/**", "*.user.yaml", "*.user.yml"]
ignore = ["logs/**", ".log/**", "cache/**", "*.bak", "*.kam-tmp"]

[dev.mcp]
enabled = true
port = 8765
local_port = 8765
endpoint = "/mcp"
transport = "streamable-http"
```

Hot sync has three default behaviors:

- `mirror`: `webroot/**` is mirrored as a directory, so stale hashed frontend
  bundles are removed from the device.
- `overlay`: allowlisted scripts, binaries, templates, `system.prop`, and
  `sepolicy.rule` are copied over existing files.
- `preserve` / `ignore`: runtime config, subscriptions, logs, cache files, and
  temporary files are not overwritten accidentally.

Kam stages files under `[dev.sync].stage_dir`, applies them from a root shell,
and backs up replaced device paths to `<path>.bak`. The default stage directory
is under `/sdcard/Download` because some Android ROMs block adb shell and even
restricted root domains from using `/data/local/tmp`.

The latest dev plan and stage summary are recorded at
`.kam/dev/last-session.log`. `kam dev --logs` prints this log, common manager
install logs, configured device logs, and recent filtered `logcat` lines.

`kam install --adb` uses the same device-friendly staging rule for module ZIPs:
it creates `/sdcard/kam/tmp` and pushes the package there before invoking the
selected root manager through `adb shell su -c`.

## Workspace Builds

Kam can build multiple modules in one workspace:

```toml
[kam.workspace]
members = [
  ".",
  "modules/module_a",
  "modules/module_b",
]
```

Build all members:

```bash
kam build --all
```

## Build Configuration

Common build options in `kam.toml`:

```toml
[kam.build]
target_dir = "dist"
output_file = "{{id}}"
hooks_dir = "hooks"
source_dir = "src/{{id}}"
include = []
respect_gitignore = false
```

Put packaging ignore rules in `.kamignore` beside the packaged root:

```gitignore
target/
node_modules/
.DS_Store
Thumbs.db
*.tmp
*.log
*.bak
.kam/
!important.log
```

Module ZIP builds read `.kamignore` from `source_dir`. Template archive builds
read `.kamignore` from the template project root. `.gitignore` is still not used
for packaging because Git development ignores and module release contents are
different concerns.

Existing `exclude` and `include` entries in `kam.toml` remain supported for
compatibility and generated overrides. `include` wins over both `.kamignore` and
`exclude`; `.kamignore` `!pattern` rules win over `.kamignore` exclude rules.

## Metadata Sync

Kam syncs `kam.toml` metadata into generated files:

- `module.prop` goes to `$KAM_MODULE_ROOT/module.prop`.
- `update.json` goes to `$KAM_PROJECT_ROOT/update.json`.
- `module.json`, `repo.json`, `track.json`, and `config.json` are generated for
  registry and marketplace use.

Run manual sync after metadata, workflow, or template baseline edits:

```bash
kam sync
kam sync workflow --source-repo owner/repo
kam sync --remote all
```

## WebUI Packaging

For modules with WebUI:

1. Develop the frontend in `webui/`.
2. Build hooks copy output into `src/<module_id>/webroot/`.
3. Kam packages `webroot/` into the module ZIP.
4. The installed module exposes it through the target manager's WebUI feature.

During development, use:

```bash
kam dev --webui --forward webui
```

## Conditional Template Logic

Declare template variables:

```toml
[kam.tmpl.variables.feature_x]
var_type = "bool"
required = false
default = false
```

Use them in template files:

```bash
{% if feature_x %}
# Feature X related code
{% endif %}
```

## Optional Online Features

Most Kam commands work offline. Network-backed features are opt-in or explicit:

- `kam tmpl pull` and `kam tmpl update` download remote template archives.
- `kam repo sync` and `kam -Sy` refresh the local module package index from
  the configured module registry. `kam -Ss` and `kam -S` read that local index;
  downloads still fetch selected release assets.
- `kam sign` does not request RFC 3161 timestamps by default; timestamping or
  Sigstore integrations may contact external services when enabled.
- Workflow commands may call GitHub or rely on GitHub Actions after files are
  committed and pushed.

## Localization CI

Kam keeps CLI and WebUI wording aligned through exported localization data.

`.github/workflows/i18n-check.yml` runs the exporter and verifies localized
`--help` output, such as `kam build --help` and `kam tmpl import --help`, under
`KAM_UI_LANGUAGE=zh` and `KAM_UI_LANGUAGE=en`.

The same workflow verifies exported JSON files are committed, preventing drift
between CLI TOML and WebUI data. To extend translations, add missing keys to
`src/i18n/en.toml`, provide translations in `src/i18n/zh.toml`, run the
exporter, and commit the resulting files.

## Kamcp

If you want interactive command help, Kamcp exposes a `kam_exec` MCP tool and
an AI assistant that can explain or run Kam commands.

See <https://github.com/MemDeco-WG/Kamcp>.
