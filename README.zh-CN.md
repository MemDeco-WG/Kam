# Kam - 模块化构建与测试工具链

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

[English](README.md) | 简体中文

! [WARNING ⚠]v1.0.0之前我不会保证api稳定性
请勿直接依赖于git版本，而是对于某一个release版本进行分叉。
并且我鼓励分叉，因为在v1.0之前我并不会保证跨版本兼容性。

## 📖 简介

Kam 是一个为 Android 模块开发者设计的端到端工具链，它统一了从项目创建、本地开发、仿真测试到真机联调的完整工作流。

###  Kam 生态系统

Kam 的核心由三大组件构成，旨在提供一个连贯而高效的开发体验：

- **kam CLI**: 核心命令行工具，是与 Kam 生态系统交互的主要入口。它负责项目脚手架、模块构建、本地仿真以及与测试环境的通信。
- **kamfw**: 一个纯 Shell 实现的轻量级模块框架。它为模块提供了标准化的生命周期、目录结构和基础功能接口，确保了模块在不同环境中的行为一致性。

## 🚀 快速上手

本章将引导你完成从安装 Kam CLI 到成功进行一次本地仿真的全过程。

### 1. 安装 kam CLI

Kam 使用 Rust 构建，推荐通过 Cargo 进行安装：

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

### kamfw: 纯 Shell 框架

`kamfw` 是 Kam 模块的心脏。它被设计成一个扁平化的纯 Shell 脚本集合，提供了模块运行所需的基础环境和核心功能库。这种设计确保了最大的兼容性和最小的依赖。

### Phase: 生命周期阶段

Kam 将模块的执行过程划分为不同的 **phase** (阶段)，例如 `post-fs-data`、`service` 等。开发者可以将自己的业务逻辑代码放置在相应的脚本文件中，`kamfw` 会在模块生命周期的正确时间点自动执行它们。


---

## 🛠️ 命令参考

以下是 `kam` CLI 的一些主要命令。

- `kam init`: 从模板创建一个新的模块项目。
- `kam build`: 构建并打包模块为可部署的 ZIP 文件。
- `kam sim`: 在本地仿真环境中运行模块。
- `kam version`: 管理模块版本号。

更多命令和详细用法，请使用 `kam --help` 查看。

## ✅ 质量门槛

在提交 PR / 合并任何变更前，请至少跑完以下命令并确保通过（将 warning 视为 error）：

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## 🤝 贡献指南

我们欢迎社区的任何贡献！无论是代码、文档还是问题反馈，都对项目至关重要。

开始贡献前请先阅读：

- [贡献指南（CONTRIBUTING.md）](CONTRIBUTING.md)
- [DEC：编码哲学与强制工程规范](.collab/decisions/DEC__coding-philosophy__20251231-1400.md)

这两份文档会说明我们的提交流程、质量门槛，以及必须遵守的工程规范（例如：禁止静默失败、Shell 输出统一、Rust 错误处理与 clone 约束等）。

## 📄 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件。
