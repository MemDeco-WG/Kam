# Kam Template System

This document explains Kam's template system, how templates are applied when you run `kam init`, the contents and formats supported, the variable replacement system, hooks, and the behavior of binary files (copied as-is during application).

This document contains two sections: English and 中文 (Simplified Chinese).

---

## English

### Overview

Kam templates package files and scaffolding used to initialize new modules. Templates are typically distributed as:

- A directory (unpacked template)
- A compressed archive: `.tar.gz`, `.tgz` (single template), or `.zip` (collection of `.tar.gz` archives or multiple templates)

Built-in templates are included in the binary as assets (`kam_template`, `meta_template`, `ak3_template`, `tmpl_template` etc.). You can also install templates to your local cache using `kam tmpl import` or download them with `kam tmpl pull`.

### Template Application (`kam init`)

`kam init` applies a template to a target path and performs variable substitution using Tera templates. The flow is:

1. Find the template source: built-in asset → local cache → local path → error.
2. If the source is an archive (.tar.gz / .tgz / .zip), Kam extracts the archive to a temporary directory.
3. It merges default variables from the template's `kam.toml` (if available) with CLI-provided `--var` options. Template-specific defaults are under the `tmpl.variables` section and are applied automatically.
4. Kam computes the final substitution variables by merging defaults, `kam.toml` derived variables (id, name, version, versionCode), and CLI `--var` values.
5. `copy_and_replace` copies files from the template into the target project directory and performs text substitution using Tera for files identified as text files.

Important: Kam only performs substitution and rendering on text files — binary files are intentionally skipped to avoid corruption (see the "Binary Files" section below). Text files are rendered using the Tera template engine with the final variable set available as context. Filenames containing `{{key}}` are also rendered.

### Supported File Formats and Import/Export

- Single template archive: `.tar.gz` or `.tgz`
- Multiple templates in a `.zip` (e.g. `templates.zip` containing multiple `.tar.gz` archives)
- Directory-based template (when using local path or installed cache entry)

Commands:

- `kam tmpl import <path>` — import a local `.tar.gz` or `.zip` file; if `.zip`, extracts `.tar.gz` entries and installs them into the local template cache (default path: `~/.kam/templates`; can be overridden by the `KAM_TEMPLATE_CACHE_DIR` environment variable or by setting `tmpl.cache_dir` in the global config `~/.kam/config.toml`).
- `kam tmpl list` — list available builtin + cached templates
- `kam tmpl export <name> -o out.tar.gz` — export a given template
- `kam tmpl pull [<url>]` — download a template ZIP from a URL and import it (`--global` configuration recorded as `tmpl.pull.url`)
- `kam tmpl update` — re-download using recorded global URL saved by `tmpl pull` and import that template ZIP again

Template authors should provide `.env.example` for recommended environment variables, and should include `hooks` under `hooks/` when they need to run scripts during `kam init` or `kam build`.

### Variables & kam.toml

A template may include a `kam.toml` used as a template scaffold. `kam.toml` can contain `tmpl.variables` which define default values for template variables. If present, Kam will merge the `tmpl.variables` defaults into the final variable set applied during `kam init`.

There are reserved variables automatically provided by Kam to templates which you can use in files and filenames:

- `id` — module id
- `name` — module name
- `version` — module version
- `versionCode` — generated version code
- `author` — module author
- `description` — module description

Additionally, you can use dot-separated keys for nested structure, and Kam will flatten them for templating. When writing templates, you can also use `{{prop.name}}` to reference nested values if you prefer.

All `kam.toml` values are available to templates as variables (typically under the `prop` object), for example:
- `{{ prop.id }}`
- `{{ prop.name }}`
- `{{ prop.version }}`

When hooks are executed during builds, Kam also exposes `kam.toml` values as environment variables using a consistent mapping:
- `prop.*` variables are exported as `KAM_PROP_*` (e.g., `prop.id` -> `KAM_PROP_ID`, `prop.name` -> `KAM_PROP_NAME`).
- All flattened keys in `kam.toml` are exported as `KAM_<PATH>` where dots and hyphens are normalized to underscores and the key is upper-cased (e.g., `mmrl.repo.repository` -> `KAM_MMRL_REPO_REPOSITORY`, `kam.build.hooks_dir` -> `KAM_KAM_BUILD_HOOKS_DIR`).
- Template variables defined under `[kam.tmpl.variables]` are exported as `KAM_TMPL_<NAME>` (upper-cased, `.`/`-` normalized to `_`).

Examples:
- Template usage: `{{ prop.id }}` and `{{ mmrl.repo.repository }}`
- Hook usage: `$KAM_PROP_ID`, `$KAM_MMRL_REPO_REPOSITORY`, or `$KAM_TMPL_FEATURE_X`

To specify values at `kam init` time, use `--var key=value` (multiple times). Example:

```bash
kam init my_mod -t kam_template --var name="MyMod" --var id=my_mod
```

and the template can reference `{{name}}`, `{{id}}` in text files and filenames.

To set values used inside Kam's generated `kam.toml` (not the template file itself), use keys that start with `#` in `--var` flags. For example, `--var #prop.version=2.0.0`.

### Binary Files

When applying templates, Kam copies binary files as-is into the target project without performing Tera variable substitution or rendering. This preserves binary content (images, archives, or compiled objects) exactly as the template provides and avoids corruption caused by templating operations.

Kam detects binary files using a simple heuristic:

- Detect a null byte within the first 1024 bytes of the file (presence indicates a binary file).
- Recognize common binary file extensions: `png, jpg, jpeg, gif, ico, zip, tar, gz, so, a, o, bin, exe`.

If a file is detected as binary, Kam will not attempt to render or substitute its content; instead, it will copy the file to the specified target path, preserving the original bytes. This behavior allows templates to include binary assets directly; they will appear in the generated module as provided by the template author.

If you need special handling for binary content, recommended approaches include:

- Place binary assets directly in the template as raw files — Kam will copy them as-is.
- Use `hooks` to perform additional operations or transformations after templating if required.
- Use `kam.kam.build.include` and `kam.kam.build.exclude` lists in `kam.toml` to control which files are copied and which are excluded during the template application process.

### Templates & Hooks

Templates may include `hooks/pre-build` and `hooks/post-build` scripts. Hooks can rely on environment variables injected by Kam (e.g. `KAM_PROJECT_ROOT`, `KAM_MODULE_ID`, `KAM_DIST_DIR`, etc.). These hooks run when you build a project (`kam build`) or initialize a template if used by the template.

Hooks execute as plain executables and Kam will not attempt to interpret the scripts- the OS handles script execution. Provide `shebang` and executable permissions for cross-platform compatibility.

### Security & File Permissions

The hook runner performs direct invocation with TLS/OS-managed interpreters. Only standard file permissions and safe path sanitization (via `strip_prefix`) are used when extracting and copying files from template archives.

### Best Practices for Template Authors

- Put all templates in `.tar.gz` format for single templates; use `.zip` to bundle multiple templates into a single file for download.
- Use `tmpl.variables` inside `kam.toml` to provide sensible defaults.
- Avoid templating in binary files. Use text-only variable substitution and provide binary assets as raw assets included in the template package.
- Keep `kam.toml` in the template describing the `tmpl` section for variables and template defaults.
- Use `hooks` sparingly; prefer to precompute artifacts and package them as part of the template.

### Troubleshooting

If rendered text contains unexpected values:
- Ensure variable keys used in templates are present in `--var` or `tmpl.variables`.
- Use `--force` to overwrite existing files in the destination when testing the template repeatedly.
- Confirm file encoding is UTF-8 where substitution is required.


---

## 中文（简体）

### 概述

Kam 模板系统用于初始化 Module 项目，将模板文件应用到目标目录，完成变量替换与结构化配置。支持的模板形式：目录、`.tar.gz`（单模板）以及`.zip`（用于包含多个 `.tar.gz` 的文件）。内置模板打包在二进制文件中（`kam_template`、`meta_template` 等）。

### `kam init` 模板应用流程

- 模板查找：内置 assets -> 本地 cache (`~/.kam/templates`，可使用环境变量 `KAM_TEMPLATE_CACHE_DIR` 覆盖) -> 本地路径 -> 若无则报错。
- 若检测到模板为归档文件（`.tar.gz`/`.tgz`/`.zip`），Kam 将解包到临时目录后再处理。
- 从模板的 `kam.toml`（若存在）加载 `tmpl.variables` 中的默认值，并与 CLI `--var` 合并。
- 最终变量集合由模板默认、CLI 提供及 `kam.toml` 推导出的内置变量（id/name/version/versionCode 等）共同构成。
- Kam 使用 `copy_and_replace` 把模板内容复制到目标目录并对文本文件进行渲染（Tera）。

重要说明：Kam 仅对文本文件进行渲染与替换，二进制文件会被跳过（请参见“二进制文件”章节）。

### 文件格式与导入/导出

- 单个模板归档：`.tar.gz` 或 `.tgz`
- 多模板包（ZIP）: `.zip`，内部包含多个`.tar.gz`
- 本地目录

命令示例：

- `kam tmpl import 模板路径` — 导入单个模板或 ZIP（会自动安装到 cache）
- `kam tmpl pull [URL]` — 从 URL 下载并导入模板 ZIP，并把 URL 记录到全局配置 (`~/.kam/config.toml`) 中
- `kam tmpl update` — 根据记录的 URL 重新下载并导入模板
- `kam tmpl list` — 列出内置 + 缓存模板
- `kam tmpl export` — 导出模板

### 变量与 `kam.toml`

模板可以在 `kam.toml` 中声明 `tmpl.variables` 以提供默认值；在调用 `kam init` 时可使用 `--var key=value` 覆盖或设置变量。在模板中可以直接使用 `{{name}}`、`{{id}}` 等占位符，也支持 `{{prop.name}}` 等点分命名。

如果需要在 Kam 生成的 `kam.toml` 中设置值，可以使用 `--var` 以 `#` 前缀作为 kam.toml 的键，例如 `--var #prop.version=2.0.0`。

### 二进制文件

Kam 在模板应用过程中会跳过二进制文件以避免损坏文件内容。二进制检测规则：

- 在文件前 1024 字节内检测到 `NULL` 字节（` `）即视为二进制；
- 或者扩展名在 `png, jpg, jpeg, gif, ico, zip, tar, gz, so, a, o, bin, exe` 列表中。

如果你的模板包含二进制资源，请将二进制文件作为资源直接打包在模板中，或通过钩子在 `init` 后运行额外的复制操作。二进制文件不做 Tera 渲染。

### Hooks（钩子）

模板可以包含 `hooks/pre-build` 与 `hooks/post-build` 目录，内含脚本可在相应阶段运行。钩子可使用 Kam 注入的环境变量（例如：`KAM_PROJECT_ROOT`、`KAM_MODULE_ID`、`KAM_DIST_DIR` 等），并在构建过程中访问这些变量。

务必确保钩子脚本具备可执行权限且包含正确的 `shebang`（例如 `#!/bin/sh`）。

### 模板作者技巧

- 为模板提供 `tmpl.variables` 默认值，便于覆盖与文档化；
- 模板中尽量只使用文本模板替换变量，避免在二进制文件中插值；
- 若需包含二进制文件，可考虑提供二进制资源到缓存目录或通过钩子处理；
- 提供 `.env.example`，以及 README 说明如何使用模板；
- 使用 `kam tmpl import` 以分发模板包，并在 `kam tmpl` 命令中记录 URL（`tmpl pull` 会记录 `tmpl.pull.url`）以便后续 `kam tmpl update` 重新下载。

---

If you want me to expand on specific parts (e.g., a template author guide, the Tera context keys available, example template layout, or step-by-step instructions for creating templates), tell me what you'd like to add and I will expand the doc accordingly.
