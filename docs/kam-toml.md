# Kam TOML 规范

`kam.toml` 是 Kam 项目的主配置文件，也是 `module.prop`、`update.json`、
仓库元数据和构建配置的统一来源。实现上的权威结构在
`src/types/kam_toml/`，本文档描述当前稳定可用的配置约定。

## 基本规则

- 文件名固定为 `kam.toml`，放在项目根目录。
- TOML 键名区分大小写。已有 Android 生态字段保持原名，例如
  `versionCode` 和 `updateJson`。
- 路径默认相对项目根目录解析；绝对路径只应在本地配置中使用，不建议写入模板。
- 缺省值由 Kam 的类型默认值和初始化流程补全；模板作者仍应把关键字段显式写出，方便使用者审阅。
- `kam validate` 是项目级配置检查入口；`kam toml get/set/unset/list` 是点路径编辑入口。

## `[prop]`

必填。对应模块基础元数据，并会同步到 `module.prop`。

| 字段 | 类型 | 必需 | 说明 |
| --- | --- | --- | --- |
| `id` | string | 是 | 模块 ID。只能包含 ASCII 字母、数字、`_`、`-`、`.`。默认源码目录使用 `src/<id>`。 |
| `name` | string | 是 | 展示名称。 |
| `version` | string | 是 | 展示版本。建议使用 `v1.0.0` 或 `1.0.0` 并在项目内保持一致。 |
| `versionCode` | integer | 是 | 单调递增的内部版本号，必须大于 0。`kam init` 会用当前时间戳生成。 |
| `author` | string | 否 | 作者或维护者。缺失时 `kam validate` 会警告。 |
| `description` | string | 是 | 模块描述，不能为空。 |
| `updateJson` | string | 否 | 更新源 URL，会同步到 `module.prop`。 |
| `metamodule` | bool/int/string | 否 | Meta module 标记。接受 `true`/`false`、`1`/`0`、`"true"`/`"false"`、`"1"`/`"0"`。 |

示例：

```toml
[prop]
id = "my_module"
name = "My Module"
version = "v1.0.0"
versionCode = 1
author = "YourName"
description = "Describe what this module does"
updateJson = "https://example.com/update.json"
metamodule = false
```

## `[kam]`

Kam 自身的项目配置。

| 字段 | 类型 | 默认 | 说明 |
| --- | --- | --- | --- |
| `min_api` | integer | `0` | Kam API 最低版本。`0` 表示未限制。 |
| `max_api` | integer | `0` | Kam API 最高版本。`0` 表示未限制。 |
| `supported_arch` | string array | `[]` | 支持架构。内置规范化值为 `arm`、`arm64`、`x86`、`x86_64`，也接受别名如 `aarch64`、`amd64`。 |
| `conflicts` | string array | `[]` | 冲突模块 ID 列表。 |
| `module_type` | string | `"kam"` | `kam` 表示可构建模块，`template` 表示模板包。模板包构建时不运行 pre/post build hooks。 |

## `[kam.build]`

构建与打包配置。

| 字段 | 类型 | 默认 | 说明 |
| --- | --- | --- | --- |
| `source_dir` | string | `src/{{id}}` | 模块源码目录。初始化后一般写成 `src/<id>`。模块 ZIP 会把该目录内容打包到 ZIP 根目录。 |
| `target_dir` | string | `dist` | 输出目录。相对路径基于项目根目录。 |
| `output_file` | string | `{{id}}-{{versionCode}}-{{version}}` | 输出文件名，不含扩展名。构建时支持 `{{id}}`、`{{name}}`、`{{version}}`、`{{versionCode}}`。 |
| `hooks_dir` | string | `hooks` | hook 根目录。 |
| `exclude` | string array | 常见临时目录和文件 | 兼容字段。构建或模板复制时排除的路径/模式；日常排除规则建议写入 `.kamignore`。 |
| `include` | string array | `[]` | 强制包含的路径/模式。匹配时优先级高于 `.kamignore` 和 `exclude`。 |
| `respect_gitignore` | bool | `false` | 保留字段。当前打包流程明确不依赖 `.gitignore`，应使用 `.kamignore` 控制产物。 |
| `extra_includes` | array of table | 无 | 额外包含项，结构为 `{ source, dest }`。 |

`.kamignore` 使用与 Kam `exclude` 相同的模式语义，一行一个规则，支持空行、`#` 注释和 `!pattern` 重新包含。模块 ZIP 构建从 `source_dir` 读取 `.kamignore`；模板归档构建从模板项目根读取 `.kamignore`。`.gitignore` 不作为打包过滤来源。

模块目录兼容性建议：

- `action.sh` 需要 ShiroSU、Magisk 27008+、KernelSU 1.0.2+ 或 APatch 11039+；低版本不支持按钮入口，应通过文档和 `mmrl.repo.manager.*.require` 表达最低版本，而不是伪装兼容。
- `post-mount.sh`、`boot-completed.sh` 面向 ShiroSU、KernelSU、APatch；Magisk 兼容项目可在 `service.sh` 里主动执行 `boot-completed.sh`。
- 仅支持 ShiroSU、KernelSU、APatch 的模块通常不需要 `META-INF/`。
- `zygisk/`、`bin/` 可以手写，但更推荐由 `c++_native/`、`go_native/` 项目按规范生成。

## `[kam.tmpl]`

模板来源与模板变量定义。

| 字段 | 类型 | 默认 | 说明 |
| --- | --- | --- | --- |
| `used_template` | string | `kam_template` | 项目由哪个模板初始化。初始化后 Kam 会写入实际模板 ID。 |
| `variables` | table | `{}` | 模板变量定义表，键为变量名。 |

变量定义格式：

```toml
[kam.tmpl.variables.repository]
var_type = "string"
required = false
default = ""
note = "GitHub repository URL (optional)"
help = "The GitHub repository URL where your module source code is hosted"
example = "https://github.com/user/repo"
choices = ["https://github.com/user/repo"]
```

字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `var_type` | string | 约定类型，例如 `string`、`bool`、`number`。当前渲染上下文按字符串传入。 |
| `required` | bool | 是否必需。 |
| `default` | string | 未通过 `--var` 提供时使用的默认值。 |
| `note` | string | 简短提示。 |
| `help` | string | 详细说明。 |
| `example` | string | 示例值。 |
| `choices` | string array | 可选候选值。 |

## `[kam.workspace]`

工作区配置。

| 字段 | 类型 | 默认 | 说明 |
| --- | --- | --- | --- |
| `members` | string array | `["."]` | 工作区成员路径。`kam build --all` 会按成员构建。 |
| `exclude` | string array | 无 | 从工作区排除的路径。 |

## `[mmrl.repo]`

发布、展示和仓库索引用元数据。字段全部可选，但模板建议显式给出常用字段。

常用字段：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `license` | string | SPDX 许可证 ID，例如 `MIT`。 |
| `license_file` | string | 许可证文件路径，例如 `LICENSE`。配置后 `kam validate` 会检查文件是否存在。 |
| `homepage` | string | 项目主页。 |
| `readme` / `readme_file` | string | README URL 或本地 README 文件。 |
| `changelog` / `changelog_file` | string | Changelog URL 或本地文件。 |
| `repository` | string | 源码仓库 URL。 |
| `documentation` | string | 外部文档 URL。 |
| `issues` | string | 问题反馈 URL。 |
| `funding` / `support` / `donate` | string | 资助、支持和捐赠入口。 |
| `cover` / `icon` | string | 展示图片或图标。 |
| `screenshots` | string array | 截图 URL 列表。 |
| `categories` / `keywords` / `features` | string array | 分类、搜索关键词和功能标签。 |
| `maintainers` | array | 可写字符串，也可写对象 `{ name, link, type }`。 |
| `devices` / `arch` / `require` | string array | 设备、架构和依赖约束。 |
| `antifeatures` | string array | 反特性标签。 |
| `min_api` / `max_api` | integer | API 约束。 |
| `verified` | bool | 是否验证。 |
| `max_num` | integer | 上层场景使用的数量限制，`0` 表示未设置。 |

子节：

```toml
[mmrl.repo.note]
title = ""
message = ""

[mmrl.repo.manager.magisk]
devices = []
arch = []
require = []

[mmrl.repo.manager.kernelsu]
devices = []
arch = []
require = []

[mmrl.repo.manager.apatch]
devices = []
arch = []
require = []

[mmrl.repo.options.archive]
compression = ""
```

## `[tool]`

预留给项目工具的自定义配置。目前稳定字段只有 `data`，建议工具把自定义配置放在 `tool.data` 下，避免污染 Kam 核心字段：

```toml
[tool.data.my_tool]
enabled = true
```

## `[rules]`

项目级检查规则覆盖。键为规则 ID，值为规则配置。

```toml
[rules.trailing_whitespace]
enabled = true
fix = true
```

| 字段 | 类型 | 默认 | 说明 |
| --- | --- | --- | --- |
| `enabled` | bool | `true` | 是否启用该规则。 |
| `fix` | bool | `true` | 使用 `--fix` 时是否允许自动修复。 |

## 模板渲染变量

初始化模板时，Kam 会向 Tera 上下文写入：

- 顶层快捷变量：`id`、`name`、`project_name`、`version`、`versionCode`、`author`、`description`、`update_json`。
- 点路径变量：`prop.id`、`prop.name`、`prop.version`、`prop.versionCode`、`prop.author`、`prop.description`。
- 部分仓库字段：`mmrl.repo.repository`、`mmrl.repo.homepage`、`mmrl.repo.readme`、`mmrl.repo.documentation`、`mmrl.repo.issues`、`mmrl.repo.cover`。
- 构建字段：`kam.build.source_dir`、`kam.build.target_dir`、`kam.build.output_file`、`kam.build.hooks_dir`。
- `kam.module_type`。
- 用户传入变量：`kam init ... --var key=value`。

`--var` 中以 `#` 开头的键会直接写入 `kam.toml` 点路径，例如：

```bash
kam init my_module -t kam --var '#mmrl.repo.repository=https://github.com/user/repo'
```

## Hook 环境变量

`kam init` 会把初始化期变量写入 `.kam/template-vars.env.init`。该文件只表示初始化时的解析结果，不会在后续 `kam build` 中自动加载。

构建 hooks 会获得当前项目和 `kam.toml` 派生变量，包括：

- `KAM_PROJECT_ROOT`
- `KAM_HOOKS_ROOT`
- `KAM_MODULE_ROOT`
- `KAM_WEB_ROOT`
- `KAM_DIST_DIR`
- `KAM_MODULE_ID`
- `KAM_MODULE_VERSION`
- `KAM_MODULE_VERSION_CODE`
- `KAM_MODULE_NAME`
- `KAM_MODULE_AUTHOR`
- `KAM_MODULE_DESCRIPTION`
- `KAM_MODULE_UPDATE_JSON`
- `KAM_STAGE`
- `KAM_<PATH>`，例如 `prop.id` 会导出为 `KAM_PROP_ID`
- `KAM_TMPL_<NAME>`，来自 `[kam.tmpl.variables]`

运行期要覆盖 hook 环境变量时，使用项目 `.env` 或 Kam 全局配置，不要修改 `.kam/template-vars.env.init`。
