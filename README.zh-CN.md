# Kam - KernelSU / APatch / Magisk / AnyKernel3 模块构建工具链

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

[English](README.md) | 简体中文

! [WARNING ⚠]v1.0.0之前我不会保证api稳定性
请勿直接依赖于git版本，而是对于某一个release版本进行分叉。
并且我鼓励分叉，因为在v1.0之前我并不会保证跨版本兼容性。

## 📖 简介

Kam 是一个为 Android 模块开发者设计的端到端工具链，支持 KernelSU、APatch、Magisk 和 AnyKernel3 模块工作流。它统一了从项目创建、本地开发、仿真测试到真机联调的完整流程。

### 支持的模块目标

Kam 支持常见 Android root/module 打包目标：

- **KernelSU** 模块，包括 KernelSU Modules Repo 的完整源码仓库和仅引用/元数据仓库。
- **APatch** 模块，包括 APatch 兼容的安装脚本、运行脚本和元数据。
- **Magisk** 模块，使用标准根目录 `module.prop`、安装脚本和运行脚本布局。
- **AnyKernel3** 内核模块，通过内置 `ak3_template` / `-t ak3` 模板创建。

标准 `kam_template` / `-t kam` 会生成 Magisk 风格模块 ZIP；只要模块脚本本身兼容目标管理器，它也适用于 KernelSU 和 APatch。内核 ZIP 项目应使用 AnyKernel3 模板，而不是普通用户态模块模板。

###  Kam 生态系统

Kam 的核心由三大组件构成，旨在提供一个连贯而高效的开发体验：

- **kam CLI**: 核心命令行工具，是与 Kam 生态系统交互的主要入口。它负责项目脚手架、模块构建、本地仿真以及与测试环境的通信。
- **kamfw**: 一个纯 Shell 实现的轻量级模块框架。它为模块提供了标准化的生命周期、目录结构和基础功能接口，确保了模块在不同环境中的行为一致性。

## 🚀 快速上手

本章将引导你完成从安装 Kam CLI 到成功进行一次本地仿真的全过程。

### 1. 安装 kam CLI

推荐使用仓库自带安装脚本，支持 macOS、Android Termux、Windows Git Bash/MSYS：

```bash
git clone https://github.com/MemDeco-WG/Kam.git
cd Kam
./install.sh
```

安装脚本会自动识别平台、检查必要工具、在缺少 `cargo` 时通过 `rustup`
安装最小 Rust 工具链、为当前 shell 配置 `~/.cargo/bin`，然后从当前源码树安装
Kam，并用 `kam --version` 验证安装结果。

平台说明：

- **macOS**：需要 `curl` 和 C 编译器。若缺少 Xcode Command Line Tools，脚本会启动
  `xcode-select --install`，完成后重新运行安装脚本。
- **Android / Termux**：会通过 `pkg install -y curl git clang make pkg-config openssl perl`
  安装构建依赖。
- **Windows**：请在 Git Bash 或 MSYS2 中运行。脚本可通过 rustup 安装 Rust；若编译
  失败，请安装 MSYS2 mingw-w64 工具链或 Visual Studio Build Tools。

也可以手动通过 Cargo 安装：

```bash
cargo install kam
```

安装完成后，通过以下命令验证是否成功：

```bash
kam --version
```

### 2. 创建你的第一个模块

使用 `kam init` 命令创建一个名为 `hello-world` 的新模块项目：

```bash
kam init hello-world -t kam_template
cd hello-world
```

这会生成! [WARNING ⚠]一个包含基本结构和配置的项目目录。

常用模板：

| 模板 | 说明 | 适用场景 |
|------|------|----------|
| `-t kam_template` (`-t kam`) | 标准 Magisk 风格模块模板 | KernelSU / APatch / Magisk 兼容模块开发 |
| `-t meta_template` (`-t meta`) | 元模块模板 | KernelSU 元数据 / metamodule 工作流 |
| `-t ak3_template` (`-t ak3`) | AnyKernel3 模板 | 内核 ZIP / AnyKernel3 模块项目 |

KernelSU Modules Repo 支持两种仓库形态，`kam init` 都可以快速生成。

完整项目仓库：模块仓库中保存源码、Kam 配置和构建 hooks：

```bash
kam init hello-world \
  --repo-mode full \
  --source-url https://github.com/you/hello-world \
  -t kam_template
```

仅引用/元数据仓库：对应 `KernelSU-Modules-Repo/org.kernelsu.example` 这类仓库，只生成
`README.md` 和 `module.json`，源码在 `sourceUrl` 指向的仓库中：

```bash
kam init hello-world \
  --repo-mode reference \
  --source-url https://github.com/you/hello-world-source \
  --project-name "Hello World" \
  --description "模块摘要"
```

为了符合 KernelSU Modules Repo 规范，仓库名必须和 `module.prop` 的 `id` 一致；
Release 应为 immutable、非 draft 的 GitHub Release；发布 ZIP 根目录必须包含
带有 `id`、`version`、`versionCode` 的 `module.prop`。


## 🧩 核心概念

理解 Kam 的核心概念将帮助你更高效地使用它。

### 模块结构

一个标准的 Kam 项目遵循以下目录结构：

```
hello-world/
├── kam.toml          # Kam 配置文件，定义模块元数据和构建选项
├── src/
│   └── hello-world/  # 模块源码目录
│       ├── module.prop   # 模块属性 (自动生成)
│       ├── customize.sh  # 安装脚本
│       └── service.sh    # 服务脚本
└── ...
```

配置与模板开发规范：

- [Kam TOML 规范](docs/kam-toml.md)：字段、类型、默认值、渲染变量和 hook 环境变量。
- [模板开发规范](docs/template-development.md)：模板目录、raw-copy 规则、打包导入和验证流程。

### kamfw: 纯 Shell 框架

`kamfw` 是 Kam 模块的心脏。它被设计成一个扁平化的纯 Shell 脚本集合，提供了模块运行所需的基础环境和核心功能库。这种设计确保了最大的兼容性和最小的依赖。

### Phase: 生命周期阶段

Kam 将模块的执行过程划分为不同的 **phase** (阶段)，例如 `post-fs-data`、`service` 等。开发者可以将自己的业务逻辑代码放置在相应的脚本文件中，`kamfw` 会在模块生命周期的正确时间点自动执行它们。


---

## 🛠️ 命令参考

以下是 `kam` CLI 的一些主要命令。

- `kam init`: 从模板创建一个新的模块项目。
- `kam add`: 在现有 Kam 项目中新增运行脚本、构建 Hook、WebUI 骨架或 kamfw helper 导入。
- `kam dev`: 启动真机开发会话，执行 dev hooks、热同步文件、转发端口、启用 MCP 并查看日志。
- `kam build`: 构建并打包模块为可部署的 ZIP 文件。
- `kam sim`: 在本地仿真环境中运行模块。
- `kam version`: 管理模块版本号。
- `kam sync`: 根据当前项目配置同步生成的元数据、GitHub Actions 工作流和可选远程模板缓存。
- `kam mcp`: 管理标准模块 MCP runtime contract。
- `kam secret ksu-generate`: 生成 KernelSU developer P-256 密钥；如可交互且系统存在 `gpg`，默认用 `gpg` 加密私钥。
- `kam secret ksu-submit`: 根据公钥生成 KernelSU developer keyring 申请 issue 表单 URL。
- `kam secret ksu-revoke`: 根据序列号或证书生成 KernelSU developer 证书吊销 issue 表单 URL。

常用新增文件命令：

```bash
kam add script service
kam add hook pre-build sync-version --order 20
kam add kamfw watchdog --phase service
kam add webui
```

常用同步命令：

```bash
kam sync
kam sync --check
kam sync workflow --source-repo LIghtJUNction/MagicNet
kam sync --remote templates
kam sync --remote all
```

`kam sync workflow --source-repo <repo>` 适合 KernelSU Modules Repo 两种仓库形态：源码仓库与当前仓库一致时安装标准校验/构建工作流；不一致时安装上游 Release 镜像同步工作流。

常用开发会话命令：

```bash
kam dev --watch --device auto
kam dev --watch --hot --mcp --logs
kam dev --webui --forward webui
kam dev --sync-only --logs
kam dev --install
kam dev --mcp
kam dev doctor
```

`kam dev` 面向迭代，不等同于 `kam build`。生产构建仍由 `kam build` 负责；开发会话使用独立 hooks：

```text
hooks/dev-build/
hooks/dev-sync/
hooks/dev-install/
hooks/dev-start/
hooks/dev-stop/
```

MCP 采用 Kam Dev Runtime Contract v1：模块根目录是 `/data/adb/modules/<id>`，标准 CLI 是 `/data/adb/modules/<id>/cli`，必须支持 `cli mcp enable|disable|status` 和 `cli mcp status --json`。MCP 通信协议为 Streamable HTTP，默认地址是 `http://127.0.0.1:8765/mcp`。`kam dev --mcp` 等价于 `kam mcp forward`、`kam mcp enable`、`kam mcp status --json`。

更多命令和详细用法，请使用 `kam --help` 查看。

## 🤝 贡献指南

我们欢迎社区的任何贡献！无论是代码、文档还是问题反馈，都对项目至关重要。

开始贡献前请先阅读：

- [贡献指南（CONTRIBUTING.md）](CONTRIBUTING.md)

## 📄 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件。
