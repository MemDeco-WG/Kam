---
agent_id: D01
task_id: DOC-TASK-0001
date: 2025-12-31
sprint: 1
---

# Kam 中文文档（README.zh-CN.md）重构补齐 - Sprint 1 报告

## 0. 背景与目标

本次 Sprint 1 的目标是：基于项目**最新架构与决策**，对 `README.zh-CN.md` 进行质量提升与内容补齐，并交付：

1) 一份新的文档大纲（目录结构）
2) 一份完整的「快速上手」章节（可直接粘贴进 README）
3) 一份需要修正的关键问题清单

> 约束：文档必须反映既定决策，包括：
> - `kamfw` 回归纯 shell 架构，并采用扁平化新脚本设计
> - `kam CLI` 的 `sim` 命令用于本地仿真测试
> - `KamModuleLab` 子模块用于真机/模拟器端到端测试
> - 严格的编码哲学（见 `.collab/decisions/DEC__coding-philosophy__20251231-1400.md`）


## 1. 现有 README.zh-CN.md 需要修正的关键问题列表

### 1.1 与“最新架构/决策”不一致或缺失

- **缺少生态三件套的清晰定位**：现有 README 主要围绕“构建/模板/元数据导出”，未将 **kam CLI / kamfw / KamModuleLab** 以统一叙事呈现。
- **未说明 kamfw 的最新定位**：文档未强调 `kamfw` 已回归**纯 shell**架构、采用**扁平化脚本设计**（这是当前强约束）。
- **缺少 `kam sim`**：现有 README 未包含 `kam sim` 的任何用法或工作流定位（本地仿真测试是指令要求）。
- **缺少 KamModuleLab**：现有 README 未介绍 `KamModuleLab` 的用途（真机/模拟器端到端测试）、基本流程与边界。
- **编码哲学未落地**：现有 README 的“贡献”章节偏通用，没有提炼并链接到当前强制规范（DEC-coding-philosophy）。

### 1.2 术语与结构问题

- **术语不够一致**：缺少对关键术语的固定解释与统一用法（kam、kamfw、KamModuleLab、phase）。
- **信息密度过高且主题混杂**：大量命令参考/高级用法/Termux 工作流放在一个 README 中，导致新手路径不清晰。

### 1.3 与代码库可见事实的偏差（需要后续确认/补齐）

> 说明：在本仓库当前可见内容中，我未能在 Rust 源码中检索到 `kam sim` 子命令的实现痕迹，且 `KamModuleLab/` 目录目前为空（无 README/脚本/代码）。
>
> 这可能表示：
> - 相关实现位于未纳入检索的子仓库/私有内容/尚未提交；或
> - 命令/模块正在开发中。
>
> **文档按任务指令要求先行补齐叙事与流程**，但建议在后续 Sprint 中：
> - 补充 `kam sim` 的真实 help 输出与示例
> - 为 `KamModuleLab` 增加最小可用说明文件（至少一个 README + 运行方式）


## 2. 新文档大纲（建议目录结构）

> 目标：将 README 改造成“新手可按步骤成功跑通”的入口文档，同时保留开发者需要的概念与链接，避免把所有细节一次性塞进 README。

```markdown
# Kam - 模块化构建与测试工具链

## 📖 简介
- Kam 是什么
- Kam 生态系统：kam CLI / kamfw / KamModuleLab
- 适用场景与非目标（边界）

## 🚀 快速上手（Sprint 1 重点）
- 安装 kam CLI
- 创建第一个 hello world 模块（kam init）
- 本地仿真：kam sim run service
- 下一步（链接到核心概念/工作流/命令参考）

## 🧩 核心概念
- 模块结构（src/<id>/…）
- kam.toml 配置（prop、构建相关、导出相关）
- kamfw：纯 shell + 扁平化脚本设计（为何如此、带来的约束）
- phase：生命周期阶段（post-fs-data/service 等）

## 🔄 开发工作流
- 本地开发与仿真（kam sim）
- 真机/模拟器端到端测试（KamModuleLab）
- 发布/分发（build/export/sign/verify 的定位）

## 🛠️ 命令参考（只列主命令，细节用 help/独立 docs）
- kam init
- kam build
- kam sim
- kam tmpl
- kam export
- kam sign / kam verify

## 🤝 贡献指南
- 贡献流程（PR/Issue）
- 编码哲学摘要（强制规范要点）
- 链接：.collab/decisions/DEC__coding-philosophy__20251231-1400.md

## 📄 许可证与致谢
```


## 3. 「快速上手」章节（完整 Markdown 内容）

> 注意：本章节按指令要求包含 `kam sim run service` 的演示流程。

```markdown
## 🚀 快速上手

本章将引导你完成从安装 `kam` CLI，到创建一个 “hello world” 模块，并使用 `kam sim run service` 完成一次本地仿真的全过程。

### 1) 安装 kam CLI

Kam 使用 Rust 构建，推荐通过 Cargo 安装：

```bash
cargo install kam
```

验证安装：

```bash
kam --version
kam --help
```

> 如果你是从源码编译：
>
> ```bash
> git clone https://github.com/MemDeco-WG/Kam.git
> cd Kam
> cargo build --release
> ./target/release/kam --version
> ```

### 2) 初始化一个 hello world 模块

使用内置模板创建新项目（以 `kam_template` 为例）：

```bash
kam init hello-world -t kam_template
cd hello-world
```

初始化后，你将看到一个基本项目结构（不同模板会略有差异）：

```text
hello-world/
├── kam.toml
├── src/
│   └── hello-world/
│       ├── customize.sh
│       ├── service.sh
│       └── ...
└── ...
```

此时建议你先打开 `kam.toml`，确认模块元信息（id/name/version 等）。

### 3) 运行本地仿真：kam sim run service

Kam 提供 `kam sim` 命令用于**本地仿真测试**，让你无需刷入真机/模拟器即可验证脚本逻辑。

在项目根目录运行：

```bash
kam sim run service
```

预期结果：
- 命令能够在本地环境中执行模块的 `service` 阶段逻辑（对应 `service.sh`）
- 你应能从输出中看到脚本被执行的日志

如果你需要进一步调试：
- 先运行 `kam sim --help` 查看可用子命令与参数
- 确认你的模块是否包含 `service.sh`（或模板约定的服务阶段入口）

### 下一步

- 想了解模块目录与配置：请阅读 **核心概念**（模块结构 / `kam.toml` / `kamfw` / phase）
- 想把模块跑到真实设备：请阅读 **开发工作流** 中的 **KamModuleLab（真机/模拟器端到端测试）**
- 想浏览命令细节：运行 `kam --help` 或查看 **命令参考**
```


## 4. 本 Sprint 已完成的改动（仓库内实际落地）

- 已将 `README.zh-CN.md` 按“生态系统 + 新手路径优先”的思路进行了重写与结构化。
- 在 README 中补齐并固定了术语：`kam`、`kamfw`、`KamModuleLab`、`phase`。
- 在贡献章节中引导读者遵循编码哲学（并建议链接到 DEC 文档）。


## 5. 风险与建议（供下一 Sprint 处理）

- 建议补充 `kam sim` 的真实命令帮助输出与最小可复现示例（以保证文档“可跑通”而非“概念性描述”）。
- 建议为 `KamModuleLab` 增加最小说明（README + 运行方式 + 示例测试），否则 README 中的叙述无法落地。
- 如需在 README 中保留大量命令细节，建议拆分到 `docs/`，README 仅保留主路径与关键链接。

