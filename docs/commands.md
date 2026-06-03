# Kam Commands Reference

This page keeps command details out of the main README so the README can stay
short and useful as an entry point.

## `kam init`

Initialize a new project from a built-in, local, archive, or cached template.

```bash
kam init [OPTIONS] [PATH]
```

Common options:

- `--id <ID>`: project id, defaulting to the folder name.
- `--project-name <NAME>`: display name.
- `--version <VERSION>`: initial version.
- `--author <AUTHOR>`: author name.
- `--description <TEXT>`: module summary.
- `--update-json <URL>`: update JSON URL.
- `-f, --force`: overwrite generated files.
- `-i, --interactive`: prompt for missing values.
- `--var <KEY=VALUE>`: pass template variables.
- `-t, --template <TEMPLATE>`: built-in alias, full template id, local
  directory, or local archive.
- `--repo-mode <full|reference>`: KernelSU Modules Repo layout.
- `--source-url <URL>`: source repository written to `module.json.sourceUrl`.
- `--metamodule`: mark generated metadata as a KernelSU metamodule.
- `--tmpl`: create a template project.

Examples:

```bash
kam init my_module -t kam
kam init my_module -t meta --interactive
kam init my_module -t ./tmpl/my_template --var repository=https://github.com/you/my_module
kam init my_module --repo-mode reference --source-url https://github.com/you/my_module-source
kam init my_template --tmpl
```

## `kam add`

Add common project parts without re-running `kam init`.

```bash
kam add <COMMAND>
```

Subcommands:

- `script <phase>`: add a runtime script under `src/<module_id>/`.
- `hook <pre-build|post-build> <name>`: add an ordered build hook under
  `hooks/`.
- `kamfw <module>`: import an existing kamfw helper into a runtime script.
- `webui`: add a static WebUI skeleton under `src/<module_id>/webroot/`.

Examples:

```bash
kam add script service
kam add script action --force
kam add hook pre-build sync-version --order 20
kam add kamfw watchdog --phase service
kam add webui
```

`kam add kamfw <module>` expects the helper to already exist in
`src/<module_id>/lib/kamfw/<module>.sh`. It inserts an `import <module>` line or
creates the target runtime script; it does not modify kamfw internals.

## `kam build`

Build and package a module into a deployable ZIP artifact.

```bash
kam build [OPTIONS] [PATH]
```

Options:

- `-a, --all`: build all workspace members.
- `-o, --output <DIR>`: output directory, defaulting to `dist`.
- `-b, --bump`: enable version bump hooks.
- `-r, --release`: enable release hooks.
- `-s, --sign`: enable signing.
- `-i, --interactive`: ask for confirmation before destructive actions.
- `-P, --pre-release`: mark release as pre-release.
- `-q, --quiet`: suppress most output.

Examples:

```bash
kam build
kam build --all
kam build --bump
kam build --release --sign
KAM_DEBUG=1 kam build
```

## `kam version`

Set or bump module versions.

```bash
kam version 1.0.1
kam version patch
kam version minor
kam version major
```

## `kam tmpl`

Manage templates.

```bash
kam tmpl list
kam tmpl import templates/meta_template.tar.gz
kam tmpl import templates.zip
kam tmpl export meta_template -o my_template.tar.gz
kam tmpl export kam_template ak3_template -o my_templates.zip
kam tmpl pull
kam tmpl pull https://example.com/templates.zip
kam tmpl update
kam tmpl remove template_name
kam tmpl path
```

Template packaging is an artifact operation. Kam does not execute build hooks
while exporting templates.

## `kam cache`

Manage local template and artifact cache.

```bash
kam cache list
kam cache clean
kam cache add ./tmpl/my_template
kam cache remove my_template
kam cache path
```

## `kam validate`

Validate `kam.toml` configuration and templates.

```bash
kam validate [PATH]
```

## `kam check`

Check project JSON, YAML, Markdown, and shell files.

```bash
kam check
kam check --json
kam check --fix
```

`kam check` requires `shellcheck` on `PATH` for shell scripts.

## `kam install`

Install a built module ZIP to a connected Android device.

```bash
kam install [OPTIONS] [PATH]
```

Options:

- `--manager <Auto|Magisk|KernelSU|APatchSU>`: preferred root manager. `Auto`
  detects the manager.
- `--adb`: install through adb and root shell.
- `--dry-run`: print the derived command without executing it.
- `-q, --quiet`: suppress non-essential output.

Examples:

```bash
kam install dist/my_module.zip
kam install --manager Auto dist/my_module.zip
kam install --adb --dry-run dist/my_module.zip
```

## `kam dev`

Start a fast Android module development session.

```bash
kam dev [OPTIONS] [COMMAND]
```

Options:

- `--device <serial|auto>`: select an adb device.
- `--watch`: poll dev paths and run incremental actions.
- `--hot`: only hot-update allowlisted files.
- `--webui`: run WebUI hooks, mirror `webroot/**`, and forward WebUI port.
- `--sync-only`: skip dev-build hooks and only sync files.
- `--install`: build a ZIP, install it, then run `hooks/dev-install/`.
- `--logs`: print local session, install, module, and filtered logcat logs.
- `--mcp`: forward, enable, and check the standard module MCP runtime.
- `--forward <mcp:webui>`: forward named endpoints.
- `--dry-run`: print planned writes and commands.

Examples:

```bash
kam dev --watch --device auto
kam dev --watch --hot --mcp --logs
kam dev --webui --forward webui
kam dev --sync-only --logs
kam dev --install
kam dev --mcp
kam dev doctor
```

## `kam diff`

Compare the installed module on an Android device with the current module
source tree. Kam pulls the installed module, filters both sides to text files,
and skips binary payloads such as images, ZIPs, APKs, `.so` files, and DEX
files.

```bash
kam diff --device auto
kam diff --device <serial>
kam diff --stat
kam diff --module-path /data/adb/modules/<module_id>
kam diff --dry-run
```

Diff direction is installed device module first, current source second, so
added lines show what exists in the local source compared with the installed
module. If direct `adb pull` cannot read the module directory, Kam retries by
creating a root-side tar archive under `/sdcard/Download/kam-diff/` and pulling
that archive instead.

## `kam mcp`

Manage the Kam Dev Runtime Contract v1 for module MCP servers.

```bash
kam mcp enable
kam mcp disable
kam mcp status
kam mcp forward
```

Contract:

- Module root: `/data/adb/modules/<module_id>`
- CLI: `/data/adb/modules/<module_id>/cli`
- Commands: `cli mcp enable`, `cli mcp disable`, `cli mcp status`,
  `cli mcp status --json`
- Transport: Streamable HTTP
- Endpoint: `http://127.0.0.1:<local_port>/mcp`

`kam dev --mcp` is equivalent to `kam mcp forward`, `kam mcp enable`, and
`kam mcp status --json`.

## `kam export`

Export `kam.toml` into generated metadata files.

```bash
kam export prop
kam export json module.json
kam export update
kam export repo
kam export track
kam export config
```

## `kam toml`

Inspect and edit TOML with dot-path keys.

```bash
kam toml get mmrl.repo.repository
kam toml set prop.name "My Module"
kam toml set prop.version=1.2.3
kam toml unset prop.not_used
kam toml list
```

## `kam config`

Manage project-local and global Kam configuration.

```bash
kam config set prop.author "YourName"
kam config --global set prop.author "YourName"
kam config get prop.author
kam config unset prop.author
kam config list
```

## `kam secret`

Manage signing and KernelSU developer secrets.

```bash
kam secret list
kam secret add main --file private_key.pem
kam secret get main
kam secret remove main
kam secret export-pub main
kam secret ksu-generate --name kernelsu-developer
kam secret ksu-submit --username <github-user> --public-key kernelsu-developer.public.pem --open
kam secret ksu-revoke --username <github-user> --serial-number <serial> --reason lost --open
```

## `kam sign`

Sign module artifacts.

```bash
kam sign module.zip
kam sign --all
kam sign --dist dist --cert cert.pem
```

## `kam sync`

Synchronize generated metadata, workflows, and optional remote template caches.

```bash
kam sync
kam sync --check
kam sync workflow --source-repo LIghtJUNction/MagicNet
kam sync --remote templates
kam sync --remote all
```

Subcommands:

- `metadata`: export metadata from `kam.toml`.
- `workflow`: reinstall GitHub Actions workflows.
- `templates`: update official template cache. Requires `--remote`.
- `all`: run metadata and workflow sync; include templates with `--remote`.

For KernelSU Modules Repo layouts, when the source repo matches the current
GitHub repository, Kam installs standard validation/build workflows. When it
points to a different upstream repository, Kam installs a release-mirror
workflow.

## `kam repo` And `kam -S`

Manage the local module package index and download module release assets.

```bash
kam -Sy
kam -Ss magic
kam -Si module_id
kam -Sl
kam -Sp module_id
kam -Sw module_id
kam -S module_id
kam -Syu module_id
kam -Sc
kam repo sync
kam repo status
kam repo search magic
kam repo info module_id
kam repo list
kam repo url module_id
kam repo fetch --yes module_id
kam repo download --yes module_id
```

Pacman-style behavior:

- `kam -Sy` refreshes the local module package index and cached module metadata.
- `kam repo status` shows the local index/cache root, index path, indexed
  package count, and cached module metadata count.
- `kam -Ss <query>` searches the local synced index. It does not refresh the
  remote registry implicitly.
- `kam -Si <module_id>` shows cached package metadata for one or more modules.
- `kam -Sl [query]` lists packages from the local synced index.
- `kam -Sp <module_id>` prints the cached selected release ZIP URL without
  downloading it, equivalent to `kam repo url <module_id>`.
- `kam -Sw <module_id>` downloads the selected release ZIP into Kam's local
  package cache, equivalent to `kam repo fetch <module_id>`.
- `kam -S <module_id>` resolves module metadata from the local cache, then
  downloads the selected release asset.
- `kam -Syu <module_id>` refreshes the local index first, then downloads the
  target. Add `--yes` to skip confirmation prompts.
- `kam -Sc` removes the local module index cache and cached module detail JSON,
  equivalent to `kam cache modules clean`.

If the local index is missing or stale, run `kam -Sy` or `kam repo sync`.

## `kam install` And `kam -U`

Install a local module ZIP through the existing Kam install compatibility
layer. `kam -U` is the pacman-style alias for installing a local package file.

```bash
kam install dist/module.zip --adb --manager Auto
kam -U dist/module.zip --adb --manager Auto
kam -U dist/module.zip --dry-run
```

Behavior:

- `kam -U <zip>` delegates to `kam install <zip>`.
- `--adb`, `--manager`, `--dry-run`, `--yes`, `--quiet`, and `-v/--verbose`
  keep the same meaning as `kam install`.
- Only one local package path is accepted at a time.

## `kam installed`, `kam query`, And `kam -Q`

Query the read-only installed-module database on a connected Android device.
Kam reads `/data/adb/modules/*/module.prop` through adb root and reports
Magisk/KernelSU/APatch module metadata without touching the module files.

```bash
kam -Q
kam -Qs magic
kam -Qi module_id
kam -Qu
kam -Qm
kam -Qn
kam -Qk
kam -Qk module_id
kam -Qo /data/adb/modules/MagicNet/cli
kam -Ql module_id
kam -Qp module.zip
kam -Qpl module.zip
kam -Q --device 5596d9
kam installed list
kam installed search magic
kam installed info module_id
kam installed upgrades
kam installed foreign
kam installed native
kam installed check
kam installed check module_id
kam installed owner /data/adb/modules/MagicNet/cli
kam installed files module_id
kam installed package-info module.zip
kam installed package-files module.zip
kam query info module_id
```

Pacman-style behavior:

- `kam -Q` lists installed modules from `/data/adb/modules`.
- `kam -Qs <query>` searches installed module ids, names, versions, authors,
  and descriptions.
- `kam -Qi <module_id>` shows installed `module.prop` metadata, module state,
  and module path.
- `kam -Qu` lists installed modules whose cached repository metadata reports a
  different latest release version. It does not refresh the remote index; run
  `kam -Sy` first when you need fresh repository metadata.
- `kam -Qm` lists installed modules not present in the local cached module
  index. This is useful for modules installed from local ZIPs, development
  builds, private repositories, or manager-only sources.
- `kam -Qn` lists installed modules present in the local cached module index.
- `kam -Qk [module_id...]` checks installed module directory integrity without
  modifying the device. It verifies `module.prop` exists, is readable, contains
  required Magisk/KernelSU/APatch metadata fields, and that `id` matches the
  module directory name.
- `kam -Qo <path>` resolves a device path under `/data/adb/modules/*` to the
  installed module that owns it, using longest-prefix matching.
- `kam -Ql <module_id...>` lists files inside installed module directories.
  Quiet mode prints paths only; default output prefixes each path with the
  owning module id.
- `kam -Qp <module.zip...>` reads root `module.prop` metadata from local module
  ZIP packages before installation. It does not require adb.
- `kam -Qpl <module.zip...>` lists files inside local module ZIP packages
  before installation. Quiet mode prints ZIP entry names only; default output
  prefixes each entry with the package path.
- `--device <serial>` selects an adb device. `--device auto` keeps adb's normal
  single-device behavior.

Module state is derived from manager marker files: `disable` means disabled,
`remove` means removal is pending, otherwise the module is shown as enabled.

## `kam installed remove` And `kam -R`

Mark installed modules for removal using the standard Magisk/KernelSU/APatch
module marker file. This is the pacman-style remove path for the device-side
installed module database.

```bash
kam -R module_id --device 5596d9
kam -R module_id --dry-run
kam installed remove module_id --yes
```

Behavior:

- `kam -R <module_id>` finds the installed module under `/data/adb/modules`.
- Removal is implemented by creating `<module_path>/remove`; the manager applies
  it on reboot.
- `--dry-run` prints the marker path without changing the device.
- `--yes` skips confirmation. Without it, Kam asks before marking modules.

## `kam workflow`

Install GitHub Actions workflows.

```bash
kam workflow install https://github.com/you/my_module
```

If the source repository is the current repository, Kam installs standard build
and release workflows. If it differs, Kam installs a release-mirror workflow for
reference-only repositories.

## `kam verify`

Verify artifact signatures or Sigstore bundles.

```bash
kam verify module.zip
kam verify module.zip --sig module.zip.sig
kam verify module.zip --bundle module.zip.sigstore.json
kam verify module.zip --cert cert.pem --root root.pem
```

## `kam completions`

Generate shell completions.

```bash
kam completions bash > /etc/bash_completion.d/kam
kam completions fish -o ~/.config/fish/completions/kam.fish
kam completions zsh --install
```

## `kam about`

Show version and project information.

```bash
kam about
```
