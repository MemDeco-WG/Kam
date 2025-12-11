# Kam -基于模板的模块构建工具

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

[![Version](https://img.shields.io/badge/version-0.3.0-blue.svg)](https://github.com/MemDeco-WG/Kam)

[English](README.md) | 简体中文

## 📖 简介

Kam 是一个强大的基于模板的模块构建工具，

目前支持的模板：
- Magisk/Apu/Ksu 模块

- Anykernel3 模板

- Astrbot V5 模板(计划中)

Kam的核心职责
解析模板
初始化
构建_版本控制


### ✨ 主要特性

- 🚀 **快速初始化** - 使用多种模板快速创建新模块项目
- 🔧 **自动化构建** - 一键构建模块 ZIP 包
- 🎯 **智能同步** - 自动同步 `kam.toml` 配置到 `module.prop` 和 `update.json`
- 🪝 **钩子系统** - 支持构建前后的自定义脚本钩子
- 📦 **模板管理** - 导入、导出和共享模块模板
- 🌐 **WebUI 支持** - 内置 WebUI 构建和集成
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

使用Astrbot模块模板 (计划支持)

```bash
kam init my_astrbot_module -t astr_template
```


### 配置模块

编辑 `kam.toml` 配置文件：

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

### ⚠️ 网络与可选在线功能

Kam 以离线为优先，但支持可选的在线特性以提升安全性和便利性：

 - **时间戳签名 / Sigstore** — 当启用 `kam sign` 的时间戳或 Sigstore 功能时，Kam 可能会联系时间戳服务器 (TSA) 或 Sigstore 的在线服务来生成 RFC 3161 时间戳签名或将签名记录到透明日志 (Rekor)。启用这些功能时需要网络访问。`kam sign` 默认启用时间戳（可用 `--timestamp=false` 关闭）。
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

### 构建选项

```bash
# 基本构建
kam build

# 构建并自动升级版本号
kam build --bump

# 构建并创建 GitHub Release
kam build --release

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

## 📞 联系方式

- GitHub Issues: [https://github.com/MemDeco-WG/Kam/issues](https://github.com/MemDeco-WG/Kam/issues)
- 作者: LightJunction

---

使用 ❤️ 和 Rust 构建
