---
agent_id: D01
order_id: D01-ORDER-DOC-0002
scope: "README.zh-CN.md + AGENTS.md + KamWiki 同步（已知问题/质量门槛）"
date: 2025-12-31
---

# D01 文档 + KamWiki 兼职同步报告（任务 4）

## 0. 目标对照

本次按指令完成以下同步：

1) 主仓库 `README.zh-CN.md` 增加「已知问题（P0）」：`kam init . --tmpl` 渲染崩溃（修复前必须写）
2) 主仓库 `AGENTS.md` 明确：
   - P0 gate：`kam init . --tmpl` 必须通过（或失败必须说明）
   - Q01 越权直报机制
3) KamWiki 同步更新相同内容：QuickStart / Workflow / QualityGates / KnownIssues


## 1) 主仓库文档变更

### 1.1 README.zh-CN.md

- 变更：新增章节 `## ⚠️ 已知问题（P0）`，包含：
  - 失败命令：`./target/release/kam init . --tmpl --force`
  - 观测到的错误示例（Template render error / tera parse '__tera_one_off'）
  - 状态：修复进行中
  - 说明其为 P0 gate，需要显式标注避免误导

对应关系：
- Wiki `Known Issues` 页面同步了相同问题描述与命令。

### 1.2 AGENTS.md

- 现状检查：已包含指令要求内容：
  - `4.3 kam init . --tmpl（P0 gate）` 明确写明：失败必须说明并由 Lead 裁决
  - `3) 越权机制：Q01 的 P0 直报流程与 Lead 裁决流程`

对应关系：
- Wiki `Quality Gates` 页面同步了 P0 红线摘要与 Q01/Lead 流程。


## 2) KamWiki 变更（新增/更新文件）

> KamWiki 当前结构为 Vue/Vite 应用，`docs/` 目录目前仅有 `cli-help.md`。
> 为保持“最小但可用”，本次在 `KamWiki/docs/` 下新增 4 个中文页面（Markdown）。

### 2.1 Quick Start

- 新增：`KamWiki/docs/quick-start.zh-CN.md`
- 内容来源：主仓库 `README.zh-CN.md` → `🚀 快速上手` 章节
- 关键段落（摘录）：

```md
## 1. 安装 kam CLI
cargo install kam

## 2. 创建你的第一个模块
kam init hello-world -t kam_template
cd hello-world

## 3. 运行本地仿真
kam sim run service
```

### 2.2 Workflow

- 新增：`KamWiki/docs/workflow.zh-CN.md`
- 内容来源：主仓库 `README.zh-CN.md` → `🔄 开发工作流` + kamfw 启动流程（以代码为准）
- 强调：`.kamfwrc` 为单一事实来源；入口脚本需先 source `.kamfwrc`。

### 2.3 Quality Gates

- 新增：`KamWiki/docs/quality-gates.zh-CN.md`
- 内容来源：主仓库 `.collab/specs/SPEC__quality-gates__v1.md` + `.collab/decisions/DEC__coding-philosophy__20251231-1400.md`
- 说明：Wiki 中给出摘要与链接，遇到冲突以主仓库规范为准。

### 2.4 Known Issues

- 新增：`KamWiki/docs/known-issues.zh-CN.md`
- 必含内容：`kam init . --tmpl` 渲染崩溃（P0 gate）
- 关键段落（摘录）：

```md
## 1. `kam init . --tmpl` 渲染崩溃（P0 Gate）

./target/release/kam init . --tmpl --force

✗ Template render error: ... Failed to parse '__tera_one_off' ...

状态：修复进行中
临时规避：目前无直接命令规避；建议手动复制现有模板目录作为临时方案。
```


## 3) 与主仓库文档的一致性说明

- 命令示例已与主仓库 README/AGENTS 对齐：
  - `cargo build --release`
  - `kam --help` / `kam <subcommand> --help`
  - `./target/release/kam init . --tmpl --force`（并显式标注当前失败）


## 4) 已知限制 / 后续建议（不撒谎）

1) KamWiki 当前路由仅展示 Home/Command 页面，并不会自动渲染 `docs/*.md`。
   - 本次按“最小但可用”先把内容写入 `docs/`，便于后续 UI 集成/发布时直接接入。
2) `kam sim` 是否存在于当前 CLI help 列表中，需要 H01/SR01 再次校准。
   - Wiki quick start 已加注：若 `kam sim` 在你的版本不可用，请以 `kam --help` 为准并反馈。


## 5) 文件清单

- 主仓库：
  - `README.zh-CN.md`（新增 P0 已知问题段落）

- KamWiki：
  - `KamWiki/docs/quick-start.zh-CN.md`
  - `KamWiki/docs/workflow.zh-CN.md`
  - `KamWiki/docs/quality-gates.zh-CN.md`
  - `KamWiki/docs/known-issues.zh-CN.md`
