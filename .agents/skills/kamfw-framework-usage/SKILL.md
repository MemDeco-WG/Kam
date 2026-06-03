---
name: kamfw-framework-usage
description: Use this skill when writing, refactoring, documenting, or debugging kamfw shell framework code in Kam module templates. Applies to tmpl/kam_template/src/{{prop.id}}/lib/kamfw, .kamfwrc, import <module>, lifecycle phases, kamfw run, i18n via set_i18n/i18n/t, logging helpers, installer filters, rich panels, watchdog helpers, and module scripts such as service.sh, customize.sh, uninstall.sh, post-fs-data.sh, boot-completed.sh, action.sh, and post-mount.sh.
---

# kamfw Framework Usage

Use this skill for the shell runtime embedded in Kam module templates:
`tmpl/kam_template/src/{{prop.id}}/lib/kamfw`.

## Core Rules

- Keep kamfw pure shell and Android/root-manager compatible.
- Before adding helpers, search existing kamfw files first.
- Do not reimplement existing kamfw helpers in module scripts. If a task needs
  logging, watchdogs, file watching, notifications, cached downloads, installer
  filters, or rich output, import the matching kamfw module and extend that
  module only when the existing API is insufficient.
- Do not put reusable framework helpers in `customize.sh`, hooks, or module
  business scripts. Add `lib/kamfw/<name>.sh`, then load it with `import <name>`.
- Importing a helper must not start long-running background work. Provide an
  explicit function such as `watchdog start ...`.
- User-visible text must use `set_i18n`, `i18n`, and `t`.
- Console output should go through `print`, `info`, `warn`, `error`, or
  `success`, not raw `printf`, unless the code needs no-newline/control output.
- Check external commands with `command -v` before use. Critical missing
  dependencies should fail fast through `abort` or a clear `error`.

## Loading Model

Module scripts load:

```sh
. "$MODDIR/lib/kamfw/.kamfwrc" || exit 1
```

`.kamfwrc` initializes `MODDIR`, loads `.config/kamfw/.envrc`, defines `print`,
`abort`, `set_perm`, and imports core modules in this order:

1. `i18n`
2. `base`
3. `logging`
4. `kam`

Use `import <module>` for optional helpers:

```sh
import rich
import watchdog
```

## Lifecycle

`kam.sh` imports `__runtime__`, initializes directories, then dispatches:

```sh
kamfw run <phase> -- "$@"
```

Supported phases:

- `install`
- `post-fs-data`
- `service`
- `boot-completed`
- `uninstall`
- `action`
- `post-mount`

Default phase handlers are no-ops. Module code may override functions such as:

```sh
kamfw_phase_service() {
  import watchdog
  watchdog start net 30 'ping -c 1 -W 1 8.8.8.8 >/dev/null 2>&1'
}
```

## i18n And Logging

Register text:

```sh
set_i18n "EXAMPLE_READY" \
  "zh" "已准备: \$_1" \
  "en" "Ready: \$_1"
```

Render text:

```sh
success "$(i18n EXAMPLE_READY | t "$KAM_MODULE_ID")"
```

Logging:

- `info`, `warn`, `error`, `debug`, `success` write through the logging helper.
- `KAM_LOGLEVEL=ERROR|WARN|INFO|DEBUG` controls verbosity.
- `KAM_DEBUG=1` implies debug logging.
- `KAM_LOGFILE` overrides the log file path.

## Installer API

For install-time file selection, use:

```sh
import __installer__

install_reset_filters
install_exclude "docs/*"
install_include "bin/*"
installer check
installer run
```

Filter order is `install_exclude`, then `install_include`, then
`install_check`/`installer check`. Zip inspection needs `zipinfo` or `unzip` and
must fail clearly if required tools are missing.

## Rich Panels

For action or installer status output:

```sh
import rich

panel "Module status"
panel_row "Version" "$KAM_MODULE_VERSION"
panel_success "Ready"
panel_end
```

Do not nest panels. Keep labels short so output remains readable in manager UI
logs and non-TTY output.

## Watchdog

Use `import watchdog` for explicit monitoring helpers. It must not auto-start on
import.

```sh
import watchdog

watchdog once 'command -v sing-box >/dev/null'
watchdog start network-check 30 'ping -c 1 -W 1 8.8.8.8 >/dev/null 2>&1'
watchdog status network-check
watchdog stop network-check
```

Start long-running watchdogs only from runtime phases such as `service`, not
from install/customize paths.

## Cached Downloads

Use `import cache_download` when module runtime code needs to download a URL
only when content changes. Importing it must not download anything.

Prefer this helper over hand-written `curl` plus `sha256sum` logic. Common
triggers include downloading rule sets, geodata, remote configs, small binaries,
or "update only when hash changed" flows. Do not create one-off cache/hash
files in business scripts unless `cache_download --hash-file <file>` cannot
express the requirement.

```sh
import cache_download

cache_download "$url" "$KAM_HOME/.config/rules.dat"
cache_download --hash "$sha256" "$url" "$dest"
cache_download --hash-url "$url.sha256" "$url" "$dest"
cache_download --hash-file "$KAM_HOME/.cache/hashes/rules.sha256" "$url" "$dest"
download_if_changed "$url" "$dest"
```

The helper writes sha256 state under `$KAM_HOME/.cache/downloads` by default and
sets `KAM_CACHE_DOWNLOAD_CHANGED=1` when it replaces the destination, otherwise
`0`. Use `--hash-file <file>` or `KAM_CACHE_DOWNLOAD_STATE_DIR` for module-owned
state paths.

If a future download flow needs GitHub release digests, ETag/Last-Modified, or
archive extraction, extend `cache_download.sh` or compose it with `binstall`;
do not duplicate the same cache comparison logic elsewhere.

## Module CLI MCP Runtime

When a kamfw-backed module exposes MCP, implement it behind the standard module
CLI path:

```text
/data/adb/modules/<module_id>/cli
```

Best practice is to make `cli` a symlink or wrapper that dispatches to the real
binary under `.local/bin/`, for example:

```text
cli -> .local/bin/<module_id>-cli
```

The CLI must support:

```sh
cli mcp enable
cli mcp disable
cli mcp status
cli mcp status --json
```

Use HTTP Streamable MCP, reported as `streamable-http`, on the device loopback
address. The default endpoint is:

```text
http://127.0.0.1:8765/mcp
```

Declare the defaults in `kam.toml` when the module supports `kam dev --mcp`:

```toml
[dev.mcp]
enabled = true
port = 8765
local_port = 8765
endpoint = "/mcp"
transport = "streamable-http"
```

`cli mcp status --json` should print only JSON to stdout:

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

Keep startup, pid files, sockets, and health checks inside the module CLI or
module runtime scripts. Kam should only call the standard CLI and set adb port
forwarding.

## Validation

After kamfw changes, run the smallest checks that cover the touched helper:

```bash
shellcheck -S error -s sh 'tmpl/kam_template/src/{{prop.id}}/lib/kamfw/<helper>.sh'
cargo run -- init /tmp/kamfw-smoke -t kam --force
```

For installer behavior, create a temp module layout and source `.kamfwrc`:

```bash
tmp=$(mktemp -d /tmp/kamfw-test.XXXXXX)
mkdir -p "$tmp/out/.config/kamfw" "$tmp/out/lib"
cp -a 'tmpl/kam_template/src/{{prop.id}}/lib/kamfw' "$tmp/out/lib/kamfw"
printf 'KAMFW_DIR=%s\nKAM_MODULES=""\nKAM_HOME=%s\n' \
  "$tmp/out/lib/kamfw" "$tmp/home" > "$tmp/out/.config/kamfw/.envrc"
MODPATH="$tmp/out" sh -c '. "$0"; import watchdog; watchdog once true' \
  "$tmp/out/lib/kamfw/.kamfwrc"
```

Report commands run and any missing host tools such as `shellcheck`.
