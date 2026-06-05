# Kam 模板开发规范

Kam 模板是一个可被 `kam init` 复制、渲染并生成项目的目录或归档。模板本身也可以通过 `kam build` 打包为 `.tar.gz`，前提是模板项目的
`[kam].module_type = "template"`。

## 推荐目录结构

```text
my_template/
├── kam.toml
├── README.md
├── LICENSE
├── CHANGELOG.md
├── icon.png
├── hooks/
│   ├── pre-build/
│   └── post-build/
└── src/
    └── {{prop.id}}/
        ├── customize.sh
        ├── service.sh
        ├── action.sh
        ├── post-mount.sh
        ├── boot-completed.sh
        ├── module.prop
        └── README.md
```

模板根目录必须能独立表达自身元数据。建议所有模板都包含 `kam.toml`、`README.md`、`LICENSE` 和 `CHANGELOG.md`。

## 模块目录兼容性

模板应按目标管理器选择模块根目录内容，不要为了旧环境盲目塞入所有脚本。

- `action.sh` 的按钮入口仅在 ShiroSU 默认支持、Magisk 27008+、KernelSU 1.0.2+、APatch 11039+ 可用。低版本没有按钮能力，模板应明确最低版本要求；不要为不支持按钮的低版本做降级按钮逻辑。建议用户升级到受支持版本，非 GKI 原版 KernelSU 用户应转向兼容分支。
- 启动类业务逻辑在 `service.sh` 与 `boot-completed.sh` 中二选一即可；需要同时区分两个时机时才同时提供两个脚本。跨 Magisk、KernelSU、APatch、ShiroSU 的兼容调度和必要的脚本重命名由 kamfw 处理，模块脚本不要手写管理器分支，也不要为了兼容在 `service.sh` 中主动调用 `boot-completed.sh`。
- 仅支持 ShiroSU、KernelSU、APatch 的模板无需包含 `META-INF/`。
- `zygisk/`、`bin/` 通常由 `c++_native/` 或 `go_native/` 下的 native 项目自动生成；模板确有静态文件需求时也可以自行提供。
- `changelog.md`、`update.json` 等发布文件可按项目需要自定义名称和路径，但路径应同步写入 `kam.toml` / README。

## Native 项目约定

C++ 项目：

- 项目目录名必须与 `Android.mk` 中的 `LOCAL_MODULE` 相同，也就是最终构建出的文件名。
- 使用 `BUILD_EXECUTABLE` 时，构建产物会自动移动到模块目录的 `bin/<项目名称>/<架构>.bin`。
- 使用 `BUILD_SHARED_LIBRARY` 时，构建产物会自动移动到模块目录的 `zygisk/<架构>.so`。

Go 项目：

- 项目目录名必须与 `go.mod` 中 `module` 值的最后一段相同，也就是最终构建出的文件名。
- `arch` 文件按行列出要构建的架构，架构名称应使用 Go 支持的值，例如 `amd64`，不要写 `x86_64`。

## 模板 `kam.toml`

模板项目应声明：

```toml
[kam]
module_type = "template"

[kam.build]
target_dir = "../../templates/"
output_file = "my_template"
hooks_dir = "hooks"
include = []
respect_gitignore = false
```

将模板自身不应进入生成项目的路径写入模板根目录 `.kamignore`：

```gitignore
src/my_template/
target/
```

规则：

- `module_type = "template"` 表示构建产物是模板归档，构建时不会运行 pre/post build hooks。
- `output_file` 应等于模板 ID，方便 `kam tmpl import/export/list` 形成稳定名称。
- 模板源码中的示例模块目录通常写成 `src/{{prop.id}}/`。路径变量会被渲染。
- `.kamignore` 用来排除模板自身不应进入生成项目的文件。不要依赖 `.gitignore` 控制模板产物。
- 模板变量定义放在 `[kam.tmpl.variables.<name>]`，所有变量都应写 `var_type`、`required`，并尽量写 `default`、`help`、`example`。

## 变量与渲染

Kam 使用 Tera 渲染模板文本文件和路径。

可用变量来源：

- `kam init` 参数：`id`、`name`、`version`、`author`、`description`。
- `kam.toml` 派生变量：`prop.id`、`prop.name`、`kam.build.source_dir` 等。
- 模板变量默认值：`[kam.tmpl.variables]` 中的 `default`。
- 用户传入变量：`--var key=value`。
- `#` 前缀变量：`--var '#prop.version=v1.2.0'` 会写入生成后的 `kam.toml` 点路径。

路径渲染和内容渲染不同：

- 文件和目录名始终可以使用 `{{prop.id}}` 这类占位符。
- 文本内容只在可渲染文件里运行 Tera。
- 二进制文件直接复制。

## Raw-copy 目录

以下路径下的文件内容默认不做 Tera 渲染，只复制原文：

- `hooks/`
- `lib/`
- `.github/`

原因是这些目录常包含 shell `${var}`、GitHub Actions `${{ ... }}` 或其他与 Tera 冲突的语法。

规范：

- 如果 raw-copy 目录里的脚本需要模板变量，优先通过构建时环境变量读取，例如 `KAM_MODULE_ID`、`KAM_PROP_ID` 或 `KAM_TMPL_REPOSITORY`。
- 不要在 raw-copy 文件内容里依赖 `{{prop.id}}` 被替换。
- raw-copy 目录的路径名仍可渲染，但不建议把 hooks/lib/.github 的目录名做成变量。

## 可渲染文本文件

除 raw-copy 目录外，以下扩展名会按 Tera 文本渲染：

- `.md`
- `.toml`
- `.json`
- `.yml`
- `.yaml`
- `.txt`
- `.prop`
- `.rule`
- `.sh`
- `.tmpl`
- `.ps1`
- `.env`
- `.example`
- 无扩展名文本文件

如果渲染失败，Kam 会直接报错，不会静默复制原文件。模板作者必须修复 Tera 语法或把文件放入 raw-copy 目录。

## Include / Exclude

模板初始化和模板打包都使用 `.kamignore`，并兼容 `kam.build.exclude` / `kam.build.include`。

规则：

- `kam.build.include` 优先级最高，高于 `.kamignore` 和 `exclude`。
- `.kamignore` 支持 `#` 注释、空行和 `!pattern` 重新包含。
- `.gitignore` 不作为打包过滤来源。
- 隐藏文件默认可进入模板产物，除非被 `.kamignore` 或 `exclude` 排除。
- 模板打包会自动跳过输出目录，避免把产物打进产物。
- 模板项目应在 `.kamignore` 中显式排除临时目录、构建目录和模板自身不应复制到用户项目的目录。

推荐 `.kamignore`：

```gitignore
target/
node_modules/
.DS_Store
Thumbs.db
*.tmp
*.log
*.bak
.kam/
```

## Hook 规范

模板可以分发 hooks，但 hooks 应保持通用、幂等、可覆盖。

约定：

- `hooks/pre-build/` 在模块构建打包前运行。
- `hooks/post-build/` 在模块 ZIP 生成后运行。
- 模板类型项目打包时不运行 hooks。
- 文件名使用数字前缀控制顺序，例如 `0100.INIT.sh`、`1000.SYNC_MODULE_FILES.sh`。
- 项目自定义 hook 可以用相同阶段和相同语义替换模板 hook。
- hook 脚本应从环境变量读取项目状态，不要自行猜测目录。

常用环境变量见 [Kam TOML 规范](kam-toml.md#hook-环境变量)。

## 模板解析顺序

`kam init -t <template>` 的解析顺序：

1. 当前工作目录下的显式路径或归档。
2. 内置模板资源。
3. 项目本地 `tmpl/` 或 `templates/` 目录。
4. 全局模板缓存。
5. 如果模板名不是路径、归档且不以 `_template` 结尾，会追加 `_template` 后重试。

因此 `kam init demo -t kam` 会解析到 `kam_template`。

## 打包与导入

常用命令：

```bash
kam build path/to/my_template
kam tmpl export my_template -o my_template.tar.gz
kam tmpl import my_template.tar.gz --force
kam tmpl list
kam tmpl path
```

模板归档支持 `.tar.gz`、`.tgz` 和 `.zip`。单模板通常使用 `.tar.gz`。

## 验证流程

每次修改模板后至少跑：

```bash
kam validate path/to/template
kam build path/to/template
rm -rf /tmp/kam-template-smoke
kam init /tmp/kam-template-smoke -t path/to/template --force
kam validate /tmp/kam-template-smoke
kam build /tmp/kam-template-smoke
```

在 Kam 源码仓库中修改内置模板时，优先使用源码路径验证：

```bash
cargo run -- validate tmpl/kam_template
cargo run -- build tmpl/kam_template
rm -rf /tmp/kam-template-smoke
cargo run -- init /tmp/kam-template-smoke -t tmpl/kam_template --force
cargo run -- validate /tmp/kam-template-smoke
cargo run -- build /tmp/kam-template-smoke
```

## 发布前检查清单

- `kam.toml` 中 `module_type = "template"`。
- `output_file` 与模板 ID 一致。
- `README.md` 说明模板用途、变量和生成结构。
- `LICENSE`、`CHANGELOG.md`、`icon.png` 等展示文件路径与 `[mmrl.repo]` 一致。
- `src/{{prop.id}}/` 能渲染成实际模块目录。
- raw-copy 目录没有误用 Tera 内容变量。
- `exclude` 不会误删必要文件，`include` 只用于明确覆盖。
- 初始化出的项目 `kam.toml` 是普通模块配置，`module_type` 应为 `kam`。
- 初始化、验证、构建都在干净临时目录通过。
