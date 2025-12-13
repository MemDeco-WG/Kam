# kam.toml 规范文档

## 概述

`kam.toml` 是 Kam 项目的核心配置文件，采用 TOML 格式。它统一了模块属性（module.prop）、更新信息（update.json）以及其他元数据，采用分层结构设计，类似于 `pyproject.toml` 的格式。

## 文件位置

- 必须位于项目根目录
- 文件名必须为 `kam.toml`（小写）
- 使用 UTF-8 编码

## 基本结构

```toml
[prop]              # 必需：模块基本属性
[mmrl]              # 可选：MMRL 仓库元数据
[kam]               # 必需：Kam 平台配置
[tmpl]              # 可选：模板相关配置
[tool]              # 可选：工具扩展配置
```

---

## 1. [prop] 节

**必需节**，包含模块的基本属性信息。

### 字段说明

| 字段 | 类型 | 必需 | 默认值 | 说明 |
|------|------|------|--------|------|
| `id` | string | ✅ | - | 模块唯一标识符，仅允许：`a-z`、`A-Z`、`0-9`、`_`、`-`、`.` |
| `name` | string | ✅ | - | 模块显示名称 |
| `version` | string | ✅ | - | 版本号，建议使用语义化版本（如 `1.0.0`） |
| `versionCode` | integer | ✅ | - | 版本代码，必须为正整数，建议使用时间戳 |
| `author` | string | ⚠️ | - | 作者名称（建议填写） |
| `description` | string | ✅ | - | 模块描述 |
| `updateJson` | string | ❌ | - | 更新 JSON 文件 URL |
| `metamodule` | boolean/integer/string | ❌ | `false` | 是否为元模块，可接受：`true`/`false`、`1`/`0`、`"true"`/`"false"` |

### 示例

```toml
[prop]
id = "MyModule"
name = "My Awesome Module"
version = "1.0.0"
versionCode = 1765610327381
author = "LightJunction"
description = "A module that does something awesome"
updateJson = "https://raw.githubusercontent.com/user/repo/branch/update.json"
metamodule = false
```

---

## 2. [mmrl] 节

**可选节**，包含 MMRL（Module Manager Repository List）仓库元数据，用于模块分发和展示。

### 2.1 [mmrl.repo] 子节

仓库/发布信息，包含展示与分发相关的元数据。

#### 字段说明

| 字段 | 类型 | 必需 | 默认值 | 说明 |
|------|------|------|--------|------|
| `license` | string | ❌ | `"MIT"` | SPDX 许可证标识符（见 https://spdx.org/licenses/） |
| `license_file` | string | ❌ | `"LICENSE"` | 许可证文件相对路径 |
| `homepage` | string | ❌ | `""` | 项目主页 URL |
| `readme` | string | ❌ | `""` | README URL（如 GitHub README 链接） |
| `readme_file` | string | ❌ | `"README.md"` | README 文件相对路径 |
| `changelog` | string | ❌ | `""` | Changelog URL |
| `changelog_file` | string | ❌ | `"CHANGELOG.md"` | Changelog 文件相对路径 |
| `screenshots` | array[string] | ❌ | `[]` | 屏幕截图 URL 列表 |
| `categories` | array[string] | ❌ | `["tools"]` | 类别标签列表 |
| `keywords` | array[string] | ❌ | `["kam", "module", "android"]` | 关键字标签列表 |
| `maintainers` | array | ❌ | `[]` | 维护者列表，支持字符串或对象形式 |
| `repository` | string | ❌ | `""` | 源代码仓库地址（如 GitHub 仓库 URL） |
| `documentation` | string | ❌ | `""` | 文档链接 |
| `issues` | string | ❌ | `""` | 问题跟踪链接 |
| `funding` | string | ❌ | `""` | 资助/捐赠链接 |
| `support` | string | ❌ | `""` | 官方支持入口 |
| `donate` | string | ❌ | `""` | 捐赠链接 |
| `cover` | string | ❌ | `""` | 封面图片 URL |
| `icon` | string | ❌ | `""` | 图标 URL 或文件名 |
| `devices` | array[string] | ❌ | `[]` | 支持或针对的设备列表 |
| `arch` | array[string] | ❌ | `[]` | 支持的 CPU 架构列表（如 `arm64-v8a`） |
| `require` | array[string] | ❌ | `[]` | 运行或安装所需的其它模块/组件标识列表 |
| `antifeatures` | array[string] | ❌ | `[]` | 与模块不兼容/禁用的功能标签 |
| `max_num` | integer | ❌ | `0` | 最大数量（语义依赖于上层使用场景，0 表示未设置） |
| `min_api` | integer | ❌ | `0` | 模块所需的最小 Kam API 版本 |
| `max_api` | integer | ❌ | `0` | 模块支持的最大 Kam API 版本 |
| `verified` | boolean | ❌ | `false` | 是否经过验证 |
| `features` | array[string] | ❌ | `["module-template", "customization"]` | 模块提供的功能/特性列表 |

#### maintainers 字段格式

支持两种格式：

1. **字符串格式**（简单名称）：
   ```toml
   maintainers = ["Alice", "Bob"]
   ```

2. **对象格式**（详细信息）：
   ```toml
   [[mmrl.repo.maintainers]]
   type = "add"
   name = "Alice"
   link = "https://github.com/alice"

   [[mmrl.repo.maintainers]]
   name = "Bob"
   ```

#### 2.2 [mmrl.repo.note] 子节

提示/通知块。

| 字段 | 类型 | 必需 | 默认值 | 说明 |
|------|------|------|--------|------|
| `title` | string | ❌ | `""` | 通知标题 |
| `message` | string | ❌ | `""` | 通知正文/消息 |

#### 2.3 [mmrl.repo.manager] 子节

各种包管理器/平台的最小版本或需求配置。

##### 2.3.1 [mmrl.repo.manager.magisk] 子节

| 字段 | 类型 | 必需 | 默认值 | 说明 |
|------|------|------|--------|------|
| `min` | integer | ❌ | - | 最低兼容版本 |
| `devices` | array[string] | ❌ | `[]` | 支持的设备列表 |
| `arch` | array[string] | ❌ | `[]` | 支持的架构列表 |
| `require` | array[string] | ❌ | `[]` | 依赖的其它模块/组件标识 |

##### 2.3.2 [mmrl.repo.manager.kernelsu] 子节

同 `[mmrl.repo.manager.magisk]` 结构。

##### 2.3.3 [mmrl.repo.manager.apatch] 子节

同 `[mmrl.repo.manager.magisk]` 结构。

#### 2.4 [mmrl.repo.options.archive] 子节

归档选项。

| 字段 | 类型 | 必需 | 默认值 | 说明 |
|------|------|------|--------|------|
| `compression` | string | ❌ | `""` | 压缩方式（如 `"Deflate"`、`"Store"` 等），空字符串表示未指定 |

### 示例

```toml
[mmrl.repo]
license = "MIT"
license_file = "LICENSE"
homepage = "https://github.com/user/repo"
readme = "https://github.com/user/repo/blob/main/README.md"
readme_file = "README.md"
changelog = "https://github.com/user/repo/blob/main/CHANGELOG.md"
changelog_file = "CHANGELOG.md"
screenshots = []
categories = ["tools"]
keywords = ["kam", "module", "android"]
repository = "https://github.com/user/repo"
issues = "https://github.com/user/repo/issues"
arch = ["arm64-v8a"]
min_api = 21
max_api = 35
verified = true
features = ["module-template", "customization"]

[mmrl.repo.note]
title = ""
message = ""

[mmrl.repo.manager.magisk]
devices = []
arch = []
require = []

[mmrl.repo.options.archive]
compression = ""
```

---

## 3. [kam] 节

**必需节**，包含与 Kam 平台相关的配置。

### 字段说明

| 字段 | 类型 | 必需 | 默认值 | 说明 |
|------|------|------|--------|------|
| `min_api` | integer | ❌ | `0` | 最低兼容 API 版本（0 表示未指定或所有版本） |
| `max_api` | integer | ❌ | `0` | 最高兼容 API 版本（0 表示未指定或不限制） |
| `supported_arch` | array[string] | ❌ | `[]` | 支持的 CPU 架构列表（如 `["arm", "arm64"]`） |
| `conflicts` | array[string] | ❌ | `[]` | 与该模块冲突的模块 ID 列表 |
| `module_type` | string | ❌ | `"kam"` | 模块类型：`"kam"` 或 `"template"` |
| `build` | table | ❌ | - | 打包/构建相关配置（见 3.1） |
| `tmpl` | table | ❌ | - | 模板相关子配置（见 3.2） |
| `workspace` | table | ❌ | - | 工作区配置（见 3.3） |

### 3.1 [kam.build] 子节

打包/构建配置。

| 字段 | 类型 | 必需 | 默认值 | 说明 |
|------|------|------|--------|------|
| `source_dir` | string | ❌ | `"src/<id>"` | 自定义源代码目录（默认为 `src/<id>`） |
| `target_dir` | string | ❌ | `"dist"` | 打包输出目录 |
| `output_file` | string | ❌ | `"{{id}}-{{versionCode}}-{{version}}"` | 输出文件名（支持模板变量） |
| `hooks_dir` | string | ❌ | `"hooks"` | 钩子脚本目录 |
| `extra_includes` | array[table] | ❌ | - | 额外包含的文件列表 |
| `exclude` | array[string] | ❌ | 见下方 | 排除路径列表（支持 glob 模式） |
| `include` | array[string] | ❌ | 见下方 | 强制包含的路径列表（覆盖 exclude，支持 glob 模式） |

#### extra_includes 字段格式

```toml
[[kam.build.extra_includes]]
source = "path/to/source"
dest = "path/to/dest"
```

#### exclude 默认值

```toml
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
```

#### include 默认值

```toml
include = [
    "META-INF/",
    "system/",
    "customize.sh",
    "module.prop",
    "service.sh",
    "post-fs-data.sh",
    "uninstall.sh",
]
```

### 3.2 [kam.tmpl] 子节

模板相关配置。

| 字段 | 类型 | 必需 | 默认值 | 说明 |
|------|------|------|--------|------|
| `used_template` | string | ❌ | `"kam_template"` | 引用的内置或自定义模板 id |
| `variables` | table | ❌ | `{}` | 模板变量定义表（变量名 -> 定义） |

#### variables 字段格式

```toml
[kam.tmpl.variables.repository]
var_type = "string"
required = false
default = ""
note = "可选提示信息"
help = "更详细的帮助文本"
example = "示例值"
choices = ["选项1", "选项2"]
```

每个变量的定义包含：

| 字段 | 类型 | 必需 | 默认值 | 说明 |
|------|------|------|--------|------|
| `var_type` | string | ❌ | `"string"` | 变量类型（如 `"string"`、`"bool"`、`"number"` 等） |
| `required` | boolean | ❌ | `false` | 是否为必需变量 |
| `default` | string | ❌ | - | 可选的默认值（作为字符串表示） |
| `note` | string | ❌ | - | 可选的提示信息 |
| `help` | string | ❌ | - | 更详细的帮助文本 |
| `example` | string | ❌ | - | 示例值 |
| `choices` | array[string] | ❌ | - | 可选的枚举候选项 |

### 3.3 [kam.workspace] 子节

工作区配置（类似 Cargo workspaces）。

| 字段 | 类型 | 必需 | 默认值 | 说明 |
|------|------|------|--------|------|
| `members` | array[string] | ❌ | `["."]` | 工作区成员列表（相对于工作区根目录的路径） |
| `exclude` | array[string] | ❌ | - | 从工作区中排除的路径列表 |

### 示例

```toml
[kam]
min_api = 0
max_api = 0
supported_arch = []
conflicts = []
module_type = "kam"

[kam.build]
target_dir = "dist"
output_file = "{{id}}"
hooks_dir = "hooks"

[kam.tmpl.variables.repository]
var_type = "string"
required = false
default = ""

[kam.workspace]
members = [
    "tmpl/*_template",
    ".",
]
```

---

## 4. [tmpl] 节

**可选节**，模板相关配置（与 `[kam.tmpl]` 类似，但位于顶层）。

> **注意**：此节与 `[kam.tmpl]` 功能重叠，建议优先使用 `[kam.tmpl]`。

---

## 5. [tool] 节

**可选节**，用于工具扩展配置，允许存储任意 JSON 兼容的数据。

### 字段说明

| 字段 | 类型 | 必需 | 默认值 | 说明 |
|------|------|------|--------|------|
| `data` | any | ❌ | - | 任意 JSON 兼容的数据结构 |

### 示例

```toml
[tool]
# 可存储任意工具特定的配置
```

---

## 架构枚举值

### SupportedArch

支持的 CPU 架构值：

- `"arm"` / `"armv7"` / `"armv7l"` / `"armv6"` / `"armhf"` → `arm`
- `"arm64"` / `"aarch64"` → `arm64`
- `"x86"` / `"i386"` / `"i486"` / `"i586"` / `"i686"` → `x86`
- `"x86_64"` / `"x64"` / `"amd64"` → `x86_64`
- 其他字符串 → 作为自定义架构

### ModuleType

模块类型值：

- `"kam"`：可发布的 Kam 模块
- `"template"`：模板模块（用于生成其他模块）

---

## 模板变量

在 `output_file` 等字段中支持以下模板变量：

- `{{id}}`：模块 ID
- `{{version}}`：版本号
- `{{versionCode}}`：版本代码

---

## 验证规则

### 必需字段检查

- `[prop]` 节必须存在
- `[prop].id` 必须非空且符合字符限制
- `[prop].version` 必须非空
- `[prop].versionCode` 必须为正整数
- `[prop].description` 必须非空

### 格式验证

- `id` 仅允许：`a-z`、`A-Z`、`0-9`、`_`、`-`、`.`
- `versionCode` 必须为正整数
- `metamodule` 可接受：`true`/`false`、`1`/`0`、`"true"`/`"false"`

### 文件存在性检查

如果指定了以下字段，对应的文件必须存在：

- `license_file`
- `readme_file`
- `changelog_file`

### 警告项

以下情况会产生警告（但不阻止构建）：

- `author` 为空
- `license` 未指定
- 源代码目录不存在

---

## 完整示例

```toml
[prop]
id = "Kam"
name = "Kam"
version = "0.4.31"
versionCode = 1765610327381
author = "LightJunction"
description = "Kam — A CLI toolkit for scaffolding, building, and distributing ksu/APU/Magisk/AnyTemplate modules"
updateJson = "https://raw.githubusercontent.com/MemDeco-WG/Kam/main/update.json"
metamodule = false

[mmrl.repo]
license = "MIT"
license_file = "LICENSE"
homepage = "https://github.com/MemDeco-WG/Kam"
readme = "https://github.com/MemDeco-WG/Kam/blob/main/README.md"
readme_file = "README.md"
changelog = "https://github.com/MemDeco-WG/Kam/blob/main/CHANGELOG.md"
changelog_file = "CHANGELOG.md"
screenshots = []
categories = ["tools"]
keywords = [
    "kam",
    "module",
    "android",
    "template",
]
maintainers = []
repository = "https://github.com/MemDeco-WG/Kam"
documentation = "https://github.com/MemDeco-WG/Kam"
issues = "https://github.com/MemDeco-WG/Kam/issues"
funding = "https://github.com/sponsors/LightJunction"
support = "https://github.com/MemDeco-WG/Kam/issues"
donate = ""
cover = ""
icon = "icon.png"
devices = []
arch = ["arm64-v8a"]
require = []
antifeatures = []
max_num = 0
min_api = 21
max_api = 35
verified = true
features = [
    "module-template",
    "customization",
]

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

[kam]
min_api = 0
max_api = 0
supported_arch = []
conflicts = []
module_type = "kam"

[kam.build]
target_dir = "dist"
output_file = "{{id}}"
hooks_dir = "hooks"

[kam.tmpl.variables.repository]
var_type = "string"
required = false
default = ""

[kam.workspace]
members = [
    "tmpl/*_template",
    ".",
]

[tool]
```

---

## 版本历史

- **v1.0.0**（当前）：初始规范版本

---

## 参考资源

- [TOML 规范](https://toml.io/)
- [SPDX 许可证列表](https://spdx.org/licenses/)
- [MMRL 规范](https://github.com/ya0211/MMRL)
