# Kam -基于模板的模块构建工具

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

[English](README.md) | 简体中文

## 📖 简介

Kam 是一个用于搭建、构建和分发 ksu/APU/Magisk/AnyTemplate 模块的 CLI 工具包。它专注于快速项目初始化、可重现构建、模板管理以及为模块维护者和分发渠道提供便捷的仓库/元数据导出。

### ✨ 主要特性

- 🚀 **快速初始化** - 使用多种模板快速创建新模块项目
- 🔧 **自动化构建** - 一键构建模块 ZIP 包
- 🔒 **网络可选** - Kam 支持大多数命令的离线操作，但某些功能可能依赖网络服务以获得额外功能（见下文）
- 🎯 **智能同步** - 自动同步 `kam.toml` 配置到 `module.prop` 和 `update.json`
- ⚙️ **配置管理** - `kam config` 管理全局（`~/.kam/config.toml`）和项目级（`./.kam/config.toml`）设置，避免重复编辑
- 🗂️ **仓库和元数据导出** - 将 `kam.toml` 导出为 repo.json、module.json、track.json、config.json，用于市场或注册表
- 🪝 **钩子系统** - 支持构建前后的自定义脚本钩子
- 📦 **模板管理** - 导入、导出和共享模块模板
- 🌐 **WebUI 集成** - 内置 WebUI 构建和集成（注意：Kam 不提供运行时模块管理）
- 🔄 **版本管理** - 自动化版本号管理和发布

## 🚀 快速开始

### 安装

```bash
cargo install kam
```

或从源码编译：

```bash
git clone https://github.com/MemDeco-WG/Kam.git
cd Kam
cargo build --release
```

### 创建新模块

使用 Kam 模板创建模块：

```bash
kam tmpl list

kam init my_kam_module -t kam_template

```

使用 Meta 模板（元模块）：

```bash
kam init my_meta_module -t meta_template
```

使用 AnyKernel3 模板（内核模块）：

```bash
kam init my_kernel_module -t ak3_template
```

### Pacman 风格的顶层选项

Kam 支持类似 pacman 的顶层选项，用于直接与远程模块注册表交互（无需进入 `repo` 子命令）：

- `-Ss <关键字>` — 在远程模块注册表中搜索 `<关键字>`（例如：`kam -Ss foo`）。
- `-S <模块ID>` — 下载指定模块的最新发布 ZIP（例如：`kam -S foo`）。
- `-u`, `--update` — 在下载前刷新模块索引（等同于 `kam repo sync --force`）。
  这些选项可以组合使用，例如：`kam -Syu <模块ID>` 会先刷新索引，然后下载模块（配合 `-y` 可自动确认）。

示例：
```bash
kam -Ss some_keyword
kam -S some_module_id
kam -Syu some_module_id
```

### MCP 服务器与 AI 协助

如果你不确定如何使用某个 `kam` 命令或希望获得交互式帮助，可以运行 MCP 服务器（Kamcp）。Kamcp 暴露了 `kam_exec` 工具并提供 AI 助手，可用于解释命令或代表你运行命令。详情与安装示例请参阅：

- https://github.com/MemDeco-WG/Kamcp

示例：向 AI 询问如何使用 `kam tmpl` 等命令。


### 配置模块

编辑 `kam.toml` 配置文件，或使用 `kam config` 命令管理配置：

```toml
[prop]
id = "my_awesome_module"
name = "My Awesome Module"
version = "1.0.0"
versionCode = 1
author = "YourName"
description = "一个超棒的模块"
updateJson = "https://example.com/update.json"

[mmrl.repo]
repository = "https://github.com/username/my_awesome_module"
changelog = "https://github.com/username/my_awesome_module/blob/main/CHANGELOG.md"
```

### 管理 Kam 配置

Kam 提供了 `kam config` 命令来管理每个项目和全局配置，类似于 `git config`：

示例：

```bash
# 设置项目级配置（存储在 `./.kam/config.toml`）
kam config set prop.author "你的名字"

# 获取项目级配置
kam config get prop.author

# 设置全局配置（存储在 `~/.kam/config.toml`）
kam config --global set prop.author "你的名字"

# 列出当前目标（项目或全局）的配置
kam config list
```

这样可以避免频繁手动编辑 `kam.toml` 中应该是全局或跨项目通用的值。

### 添加模块文件

将你的模块文件添加到 `src/<module_id>/` 目录：

```
src/my_awesome_module/
├── module.prop          # 自动生成
├── customize.sh         # 安装脚本
├── service.sh           # 服务脚本
├── system/              # 系统文件
│   └── bin/
│       └── my_script
└── webroot/             # WebUI 文件（可选）
```

### 构建模块

```bash
kam build
```

构建产物将生成在 `dist/` 目录下。

## 📚 详细文档

### 模板类型

Kam 提供多种内置模板：

| 模板 | 说明 | 适用场景 |
|------|------|----------|
| `-t kam_template`（别名：`-t kam`） | 标准 Kam 模块 | 通用模块开发 |
| `-t meta_template`（别名：`-t meta`） | 元模块模板 | 元模块（模块的模块） |
| `-t ak3_template`（别名：`-t ak3`） | AnyKernel3 模板 | 内核模块 |
| `--tmpl` | 模板开发模板（映射到 `tmpl_template`） | 创建新的模板 |

### 模板管理

#### 导入模板

导入单个模板：
```bash
kam tmpl import templates/meta_template.tar.gz
```

从 ZIP 文件导入多个模板：
```bash
kam tmpl import templates/all-templates.zip
```

#### 列出可用模板

```bash
kam tmpl list
```

#### 导出模板

导出单个模板：
```bash
kam tmpl export meta_template -o my_template.tar.gz

注意：将单个模板导出为 `.tar.gz`（模板打包）时，Kam 不会执行 pre-build 或 post-build 钩子。模板打包无需执行钩子。
导出多个模板到 ZIP：
```bash
kam tmpl export kam_template ak3_template -o my_templates.zip
```

有关模板系统的详细使用说明、模板开发建议与 `kam init` 的行为（包括二进制文件跳过策略），请参阅：`docs/templates.md`。

#### 其他模板命令

```bash
# 从缓存中删除模板
kam tmpl remove template_name

# 显示模板缓存目录
kam tmpl path
```

## 📖 命令参考

### Termux（基于 SSH 的工作流）

`kam termux` 子命令现在优先使用基于 SSH 的工作流（相较于先前基于 adb-shell 的守护进程方式更安全）。常用帮助选项：

- `--ssh-setup` ：在设备上打印准备说明（安装 `openssh`、运行 `passwd` 设置密码、启动 `sshd`）。
- `--ssh-forward` ：建立 `adb forward tcp:<port> tcp:<port>`（默认端口 `8022`）。
- `--ssh-push-key <PATH>` ：使用 `adb push` 将本地公钥写入设备 `~/.ssh/authorized_keys`（尽量以安全方式进行）。
- `--ssh-connect` ：尝试确保端口转发（best-effort），并运行 `ssh -p <port> localhost`。
- `--ssh-auto` ：便捷流程：转发 + 推送默认公钥（存在时）+ 连接。
- `-i` / `--interactive` ：交互式引导流程（推荐）：
  - 优先尝试 `ssh-copy-id`（会交互提示密码）；
  - 若 `ssh-copy-id` 不可用则回退到 `scp`；
  - 若网络拷贝不可用则回退到 `adb push`（在无法直接写入 app-private 时会把公钥推到 `/sdcard` 并提示在设备上完成 append）。

快速示例

在设备（Termux）上：
```bash
pkg update && pkg upgrade
pkg install openssh
passwd    # 设置 SSH 密码
sshd      # 启动 SSH 服务（默认端口 8022）
```

在电脑上：
```bash
# 交互式引导（会尝试 ssh-copy-id / scp / adb push 回退）
kam termux -i

# 或者便捷自动化：转发 + 推送默认公钥（若存在）+ 连接
kam termux --ssh-auto

# 手动示例
kam termux --ssh-forward
ssh -p 8022 user@localhost
```

说明：
- 若本机有 `ssh-copy-id`，会优先使用（交互式输入远端密码）；若无则按上面回退顺序处理。
- 在 CI / 非交互环境下，推荐使用 `--ssh-push-key <PATH>`（显式指定公钥），或手工在设备内完成钥匙安装。
- 之前基于 adb-shell 的 `daemon/list/kill` 已移除；建议使用本节介绍的 SSH 工作流以提高安全性与可维护性。

### `kam init` - 初始化新项目

从模板初始化一个新的 Kam 项目（支持元模块和内核模块模板）。

```bash
kam init [OPTIONS] [PATH]
```

**参数：**
- `[PATH]` - 初始化项目的路径。在交互模式下（`-i`/`--interactive`），可以省略此路径；交互流程会提示输入。

**选项：**
- `--id <ID>` - 项目 ID（默认：文件夹名称）
- `--project-name <PROJECT_NAME>` - 项目名称（默认："Example Module Name"）
- `--version <VERSION>` - 项目版本（默认："1.0.0"）
- `--author <AUTHOR>` - 作者名称（默认："Your Name"）
- `--update-json <UPDATE_JSON>` - 更新 JSON URL（默认：从 git 自动生成）
- `--description <DESCRIPTION>` - 描述（默认："Describe your module here"）
- `-f, --force` - 强制覆盖现有文件
- `-i, --interactive` - 交互式运行初始化；询问必需的值
- `--var <VAR>` - 模板变量，格式为 key=value
- `-t, --template <TEMPLATE>` - 要使用的模板（内置 ID 或本地路径）
- `--tmpl` - 创建模板项目（模板 ID："tmpl_template"）

**示例：**
```bash
kam init my_module -t kam_template
kam init my_module -t meta_template --interactive
kam init my_module --tmpl
```

### `kam build` - 构建和打包模块

构建并打包模块为可部署的 ZIP 文件。

```bash
kam build [OPTIONS] [PATH]
```

**参数：**
- `[PATH]` - 项目路径（默认：当前目录）

**选项：**
- `-a, --all` - 构建所有工作空间成员
- `-o, --output <OUTPUT>` - 输出目录（默认：dist）
- `-b, --bump` - 启用 KAM_BUMP_ENABLED 环境变量（设置为 1）
- `-r, --release` - 启用 KAM_RELEASE_ENABLED 环境变量（设置为 1）
- `-s, --sign` - 启用 KAM_SIGN_ENABLE 环境变量（设置为 1）
- `-i, --interactive` - 交互式运行构建；在执行可能破坏性操作时询问确认
- `-P, --pre-release` - 启用 KAM_PRE_RELEASE 环境变量（设置为 1）
- `-q, --quiet` - 抑制大部分输出；仅显示警告和错误

**示例：**
```bash
kam build
kam build --all
kam build --bump
kam build --release --sign
kam build --interactive
```

### `kam version` - 管理模块版本

管理模块版本和版本号升级策略。

```bash
kam version [VERSION]
```

**参数：**
- `[VERSION]` - 新版本（例如 1.0.1）或升级类型（major, minor, patch）

**示例：**
```bash
kam version 1.0.1
kam version patch
kam version minor
kam version major
```

### `kam tmpl` - 模板管理

管理模板：导入、导出、打包和列出。

```bash
kam tmpl <COMMAND>
```

**子命令：**

#### `kam tmpl list` - 列出可用模板
```bash
kam tmpl list
```

#### `kam tmpl import` - 导入模板
```bash
kam tmpl import [OPTIONS] <PATH>
```
- `<PATH>` - 模板归档文件路径（单个模板为 .tar.gz，多个模板为 .zip）
- `-n, --name <NAME>` - 模板名称（可选，如未提供则使用文件名）
- `-f, --force` - 如果模板已存在则强制覆盖

#### `kam tmpl export` - 导出模板
```bash
kam tmpl export [OPTIONS] --output <OUTPUT> [TEMPLATES]...
```
- `[TEMPLATES]...` - 要导出的模板名称（可指定多个）
- `-o, --output <OUTPUT>` - 输出文件路径（单个模板为 .tar.gz，多个模板为 .zip）
- `-f, --force` - 如果输出文件已存在则强制覆盖

#### `kam tmpl pull` - 下载模板
```bash
kam tmpl pull [OPTIONS] [URL]
```
- `[URL]` - 下载 URL（默认为 GitHub 最新发布的 templates ZIP）
- `--global` - （注意：URL 始终记录在全局配置中：`~/.kam/config.toml`）`--global` 标志为 CLI 一致性而接受，但无实际效果

#### `kam tmpl update` - 更新模板
根据配置中记录的 URL 重新下载并导入。

```bash
kam tmpl update
```

#### `kam tmpl remove` - 删除模板
```bash
kam tmpl remove <TEMPLATE>
```

#### `kam tmpl path` - 显示模板缓存目录
```bash
kam tmpl path
```

### `kam cache` - 管理本地缓存

管理本地模板和构建产物缓存。

```bash
kam cache <COMMAND>
```

**子命令：**
- `kam cache list` - 列出缓存的模板
- `kam cache clean` - 清理所有缓存的模板
- `kam cache add` - 从本地目录或归档文件添加模板到缓存
- `kam cache remove` - 从缓存中删除模板
- `kam cache path` - 显示缓存目录路径

### `kam validate` - 验证配置

验证 `kam.toml` 配置和模板。

```bash
kam validate [PATH]
```

**参数：**
- `[PATH]` - 项目目录路径（默认：当前目录）

### `kam check` - 检查项目文件

检查项目中的 JSON/YAML/Markdown 文件（检查/格式化/解析）。

```bash
kam check [OPTIONS] [PATH]
```

**参数：**
- `[PATH]` - 项目目录路径（默认：当前目录）

**选项：**
- `--json` - 以 JSON 格式输出结果
- `--fix` - 尝试自动修复/格式化文件

**示例：**
```bash
kam check
kam check --json
kam check --fix
```

### `kam export` - 导出配置

将 `kam.toml` 导出为 `module.prop`、`module.json`、`repo.json`、`track.json`、`config.json`、`update.json`。

```bash
kam export [FORMAT] [OUTPUT]
```

**参数：**
- `[FORMAT]` - 导出格式：prop, json, update, repo, track, config
- `[OUTPUT]` - 输出文件路径（默认：写入当前目录中格式特定的文件名）

**示例：**
```bash
kam export prop
kam export json module.json
kam export update
kam export repo
```

### `kam toml` - TOML 操作

使用点分隔的键路径检查和编辑 `kam.toml`（get/set/unset/list）。

```bash
kam toml [OPTIONS] <COMMAND>
```

**选项：**
- `--file <FILE>` - 操作项目的 kam.toml（默认），或使用 --file 指定文件

**子命令：**
- `kam toml get <KEY>` - 通过点分隔的键路径获取值
- `kam toml set <KEY> <VALUE>` - 通过键设置值（用法：`kam toml set prop.name=value` 或 `kam toml set prop.name value`）
- `kam toml unset <KEY>` - 取消设置/删除键
- `kam toml list` - 转储完整的 toml

**示例：**
```bash
kam toml get mmrl.repo.repository
kam toml set prop.name "我的模块"
kam toml set prop.version=1.2.3
kam toml unset prop.not_used
kam toml list
```

### `kam config` - 配置管理

管理每个项目或全局的 kam 配置（类似于 git config）。

```bash
kam config [OPTIONS] <COMMAND>
```

**选项：**
- `--global` - 使用全局配置文件（`~/.kam/config.toml`）
- `--local` - 强制使用本地配置文件（项目 `.kam/config.toml`）

**子命令：**
- `kam config get <KEY>` - 通过键（点分隔路径）获取配置值
- `kam config set <KEY> <VALUE>` - 通过键设置配置值
- `kam config unset <KEY>` - 取消设置（删除）配置值
- `kam config list` - 列出目标文件中的所有配置值

**示例：**
```bash
kam config set prop.author "你的名字"
kam config --global set prop.author "你的名字"
kam config get prop.author
kam config list
```

### `kam secret` - 密钥管理

密钥管理（用于签名/验证任务）。

```bash
kam secret <COMMAND>
```

**子命令：**
- `kam secret list` - 列出已保存的密钥
- `kam secret add <NAME> [FILE]` - 从值或文件添加密钥
  - `-f, --file <FILE>` - 读取密钥的文件路径
  - `-v, --value <VALUE>` - 直接提供值
  - `--force-file` - 强制存储到本地文件而不是系统密钥环
  - `--password <PASSWORD>` - 在 CLI 上传递密码（不推荐）；如果未设置，将提示输入密码
  - `--with-backup` - 同时在 ~/.kam/secrets 下创建本地备用文件
- `kam secret get <NAME>` - 获取密钥并打印到 stdout（或 --out 文件）
- `kam secret remove <NAME>` - 删除密钥
- `kam secret export <NAME>` - 导出密钥到文件（默认解密）。使用 --encrypted 导出加密的 blob
- `kam secret import <NAME> <FILE>` - 从文件导入密钥。如果文件是加密的 KAM blob，将按原样存储
- `kam secret export-pub <NAME>` - 从存储的私钥密钥导出公钥
- `kam secret import-cert` - 从 GitHub issue 或文件导入开发者证书链
- `kam secret trust` - 管理受信任的根 CA

**示例：**
```bash
kam secret add main --file private_key.pem
kam secret list
kam secret export-pub main
```

### `kam sign` - 签名构建产物

使用密钥环中的密钥或 PEM 文件对构建产物进行签名。

```bash
kam sign [OPTIONS] [SRC]
```

**参数：**
- `[SRC]` - 要签名的构建产物（zip）。如果省略，使用 --dist 或 --all 对多个文件进行签名

**选项：**
- `--secret <SECRET>` - kam 密钥环中保存私钥的密钥名称 [默认：main]
- `--out <OUT>` - 输出目录（默认：dist）
- `--dist <DIR>` - 对给定目录中的所有构建产物进行签名（而不是指定单个 src 文件）
- `--all` - 对 dist 内的所有构建产物进行签名（使用默认 dist 的 --dist <dir> 的别名）
- `--cert <CERT>` - 要包含在签名元数据中的证书 PEM 链路径
- `--key-path <KEY_PATH>` - 可选路径，指向用于替代密钥环密钥的私钥 PEM 文件

**示例：**
```bash
kam sign module.zip
kam sign --all
kam sign --dist dist --cert cert.pem
```

### `kam verify` - 验证签名

验证构建产物签名（.sig）或 sigstore 包（DSSE）。

```bash
kam verify [OPTIONS] [SRC]
```

**参数：**
- `[SRC]` - 要验证的构建产物路径（.sig 验证必需）

**选项：**
- `--sig <SIG>` - 签名文件路径（base64 .sig）。如果省略，默认为 <src>.sig
- `--bundle <BUNDLE>` - 包含 DSSE 信封和证书的 .sigstore.json 包路径
- `--cert <CERT>` - 用于验证的可选证书 PEM（覆盖包证书）
- `--root <ROOT>` - 用于验证证书链的可选根 CA PEM（受信任锚点）
- `--secret <SECRET>` - kam 密钥环中保存私钥的密钥名称；用于派生公钥进行验证 [默认：main]
- `--key <KEY>` - 用于验证的公钥 PEM 路径（覆盖从密钥派生的密钥）
- `--cert-name <CERT_NAME>` - 用于验证的缓存开发者证书名称
- `--cert-chain <CERT_CHAIN>` - 用于验证的证书链 PEM 文件路径
- `--skip-crl` - 跳过 CRL（证书撤销列表）检查
- `-v, --verbose` - 显示验证步骤的详细输出

**示例：**
```bash
kam verify module.zip
kam verify module.zip --sig module.zip.sig
kam verify module.zip --bundle module.zip.sigstore.json
kam verify module.zip --cert cert.pem --root root.pem
```

### `kam completions` - 生成 Shell 补全

为常见 shell 生成补全脚本。

```bash
kam completions [OPTIONS] <SHELL>
```

**参数：**
- `<SHELL>` - 补全的 shell 类型（bash, zsh, fish, powershell, elvish）

**选项：**
- `-o, --out <OUT>` - 输出文件。如果省略，打印到 STDOUT
- `--install` - 将补全脚本安装到标准 shell 补全目录（可能需要 root 权限）

**示例：**
```bash
kam completions bash > /etc/bash_completion.d/kam
kam completions fish -o ~/.config/fish/completions/kam.fish
kam completions zsh --install
```

### `kam about` - 显示关于信息

显示 Kam 的关于信息和致谢。

```bash
kam about
```

### ⚠️ 网络与可选在线功能

Kam 以离线为优先，但支持可选的在线特性以提升安全性和便利性：

 - **时间戳签名 / Sigstore** — 当启用 `kam sign` 的时间戳或 Sigstore 功能时，Kam 可能会联系时间戳服务器 (TSA) 或 Sigstore 的在线服务来生成 RFC 3161 时间戳签名或将签名记录到透明日志 (Rekor)。启用这些功能时需要网络访问。`kam sign` 默认不启用时间戳（可用 `--timestamp` 启用）。
- **模板下载（已实现）** — 新增 `kam tmpl pull` 命令，方便从远程仓库或模板注册表拉取并导入模板（默认为 GitHub latest release templates.zip）。
  已记录的下载链接保存在全局配置 `~/.kam/config.toml` 下的 `tmpl.pull.url`；最近一次下载时间保存在 `tmpl.pull.last_download`。

示例：
```bash
# 使用默认地址（将被记录到全局配置）
kam tmpl pull

# 指定 URL 并记录到全局配置
kam tmpl pull https://example.com/templates.zip

# 使用已记录在全局配置的链接重新下载并导入
kam tmpl update
```

这些功能默认情况下尽量关闭，以保留 Kam 的离线优先特性。

### TOML 操作

你可以使用 `toml` 子命令直接从 CLI 检查和修改 `kam.toml`：

```bash
# 通过点分隔的键路径获取嵌套值
kam toml get mmrl.repo.repository

# 设置值：支持 `key value` 和 `key=value` 两种形式
kam toml set prop.name "我的模块"
kam toml set prop.version=1.2.3

# 删除值
kam toml unset prop.not_used

# 转储 kam.toml
kam toml list
```

### 构建选项

```bash
# 基本构建
kam build

# 构建所有
kam build -a    # --all 的简写
kam build --all

# 构建并自动升级版本号
kam build --bump

# 构建并创建 GitHub Release
# 创建 GitHub release 并从 `dist/` 上传构建产物（签名和不可变性可选）
kam build --release
#
# 示例：创建不可变的签名 release（如果相同标签已存在则跳过重新上传）并上传 Sigstore
# 证明 JSON（DSSE 包复制为 `*.attestation.json`）作为 release 资产：
#
# kam build -r -s -i

# 调试模式
KAM_DEBUG=1 kam build
```

### 检查项目文件

验证项目中的常见数据文件（JSON、YAML、Markdown）。该命令会检查解析错误以及基本的格式问题；使用 `--fix` 尝试自动修复文件。

```bash
# 检查当前目录并以人类可读方式输出
kam check

# 以 JSON 格式输出结果
kam check --json

# 尝试自动修复 / 格式化文件
kam check --fix
```

### 钩子系统

Kam 支持在构建过程中执行自定义脚本：

#### Pre-build 钩子（构建前）

在 `hooks/pre-build/` 目录下创建脚本：

```bash
hooks/pre-build/
├── 0.EXAMPLE.sh              # 示例 pre-build 钩子 (模板)
├── 1.SYNC_MODULE_FILES.sh    # 同步配置文件 (脚本)
├── 2.BUILD_WEBUI.sh          # 构建 WebUI

```

#### Post-build 钩子（构建后）

在 `hooks/post-build/` 目录下创建脚本：

```bash
hooks/post-build/
├── 0.EXAMPLE.sh                # 示例 post-build 钩子 (模板)

#### 可用的环境变量

| 变量 | 说明 |
| `KAM_PROJECT_ROOT` | 项目根目录绝对路径 |
| `KAM_HOOKS_ROOT` | 钩子目录绝对路径 |
| `KAM_MODULE_ROOT` | 模块源目录绝对路径（如 `src/<id>`） |
| `KAM_WEB_ROOT` | 模块 webroot 目录绝对路径 |
| `KAM_DIST_DIR` | 构建输出目录绝对路径（如 `dist`） |
| `KAM_MODULE_ID` | 模块 ID |
| `KAM_MODULE_VERSION` | 模块版本 |
| `KAM_MODULE_VERSION_CODE` | 模块版本号 |
| `KAM_MODULE_NAME` | 模块名称 |
| `KAM_MODULE_AUTHOR` | 模块作者 |
| `KAM_MODULE_DESCRIPTION` | 模块描述 |
| `KAM_MODULE_UPDATE_JSON` | 模块 updateJson URL |
| `KAM_STAGE` | 当前构建阶段：`pre-build` 或 `post-build` |
| `KAM_DEBUG` | 设置为 `1` 启用调试输出 |

注意：当将单个模板导出为 `.tar.gz`（模板打包）时，Kam 不会执行 pre-build 或 post-build 钩子。模板打包通常不需要执行钩子。

### 自动同步

Kam 会自动同步 `kam.toml` 的配置到模块文件：

- **module.prop** → `$KAM_MODULE_ROOT/module.prop`
  - 包含模块元数据（id、name、version 等）

- **update.json** → `$KAM_PROJECT_ROOT/update.json`
  - 包含更新信息（version、versionCode、zipUrl、changelog）
  - 自动从 `[mmrl.repo]` 推断 URL

### WebUI 集成

Kam 支持为模块添加 WebUI 界面：

1. 在 `webui/` 目录下开发你的前端应用
2. WebUI 会自动构建并安装到 `src/<module_id>/webroot/`
3. 模块安装后可通过管理器的 WebUI 功能访问

## 🔧 高级用法

### 工作空间（Workspace）

Kam 支持工作空间模式，可以在一个项目中管理多个模块：

```toml
[kam.workspace]
members = [
    ".",
    "modules/module_a",
    "modules/module_b",
]
```

### 自定义构建配置

```toml
[kam.build]
target_dir = "dist"              # 输出目录
output_file = "{{id}}"           # 输出文件名模板
hooks_dir = "hooks"              # 钩子目录
source_dir = "src/{{id}}"        # 源码目录（可选）
```

### 条件编译

使用模板变量实现条件编译：

```toml
[kam.tmpl.variables.feature_x]
var_type = "bool"
required = false
default = false
```

在脚本中使用：

```bash
{% if feature_x %}
# Feature X 相关代码
{% endif %}
```

## 📋 项目结构

```
my_module/
├── kam.toml                    # Kam 配置文件
├── src/
│   └── my_module/              # 模块源码
│       ├── module.prop         # 模块属性（自动生成）
│       ├── customize.sh        # 安装脚本
│       ├── service.sh          # 服务脚本
│       └── system/             # 系统文件
├── hooks/
│   ├── pre-build/              # 构建前钩子
│   └── post-build/             # 构建后钩子
├── webui/                      # WebUI 源码（可选）
├── dist/                       # 构建输出
├── update.json                 # 更新信息（自动生成）
└── README.md
```

## 🤝 贡献

欢迎贡献代码、报告问题或提出建议！

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add some amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

## 📄 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件

## 🙏 致谢

- [Magisk](https://github.com/topjohnwu/Magisk) - Android 的魔法框架
- [KernelSU](https://github.com/tiann/KernelSU) - 基于内核的 root 方案
- [APatch](https://github.com/bmax121/APatch) - 另一个内核 root 方案
- [Mmrl](https://github.com/MMRLApp/MMRL) - 模块仓库

## 📞 联系方式

- GitHub Issues: [https://github.com/MemDeco-WG/Kam/issues](https://github.com/MemDeco-WG/Kam/issues)
- 作者: LightJunction

---

使用 ❤️ 和 Rust 构建
