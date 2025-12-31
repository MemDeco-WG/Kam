# 贡献指南（CONTRIBUTING）

本文档说明如何为 Kam 贡献代码、文档与翻译，并列出必须遵守的工程规范。

> 强制规范（必须遵守）：
> - [DEC：编码哲学与强制工程规范](.collab/decisions/DEC__coding-philosophy__20251231-1400.md)
>
> 如果本文与 DEC 存在冲突，以 DEC 为准。


## 1. 你可以贡献什么

- **代码贡献**：核心 CLI（Rust）、模板与脚本（Shell）、构建/钩子系统等
- **文档贡献**：README、KamWiki、命令示例与运行手册
- **翻译贡献**：本地化文案与文档修订
- **问题反馈**：Bug、兼容性、使用体验、提案（RFC/DEC）


## 2. 开发与提交流程（标准路径）

1. Fork 本仓库并创建分支：
   ```bash
   git checkout -b feat/<topic>
   ```
2. 进行修改（尽量小步提交，保持变更聚焦）。
3. 确保变更满足“质量门槛”（见第 3 节）。
4. 提交：
   ```bash
   git commit -m "<type>: <summary>"
   ```
5. Push 并创建 Pull Request，描述：
   - 你解决了什么问题
   - 你做了哪些设计选择（尤其是错误处理与兼容性）
   - 如果涉及行为变更，请提供验证方式/复现步骤


## 3. 质量门槛（PR 必查清单）

本节是对 DEC 的“可执行摘要”。PR 作者与 Reviewer 都应据此检查。

### 3.1 禁止静默失败 / 隐式回退（强制）

- **宁愿明确报错并中止，也不要**返回“看起来正确”的默认值。
- 禁止：
  - Shell：不加说明的 `|| true`、吞错后继续、用 `2>/dev/null` 掩盖关键错误
  - Rust：`unwrap_or/unwrap_or_default` 用默认值掩盖错误、`_ => ...` 隐藏未覆盖分支

你需要做的是：
- 明确返回错误（Rust：`Result<T, E>` 上抛；Shell：明确 `return 1`/`exit 1`）
- 或在“逻辑不可达”处使用 `panic!/unreachable!` 并写清原因（仅限确实不可达）

### 3.2 Rust 工程规范（强制）

- **错误处理**：避免 `unwrap/expect`（测试或不可达除外）；应用优先 `anyhow`，库优先 `thiserror`
- **减少分配**：优先 `&str` / `&[T]`，必要时使用 `Cow`
- **禁止滥用 `.clone()` / `.to_string()`**：如必须使用，请在 PR 中解释原因
- **迭代器优先**：能用 iterator chain 就不要索引循环

### 3.3 Shell（kamfw）工程规范（强制）

- **输出统一**：用户可见输出必须走 `.kamfwrc` 定义的 `print/ui_print`（禁止直接 `echo` 作为用户输出）
- **避免复制粘贴式 fallback**：重复逻辑必须抽象复用（例如统一的 print/error/abort）
- **ShellCheck 语境**：脚本头必须声明：
  ```sh
  # shellcheck shell=ash
  ```

> 提示：如果你在模板或 `kamfw` 里新增脚本，请优先遵循现有的工具函数与输出风格。


## 4. 文档贡献（README / KamWiki）

- README 应聚焦“新手可跑通”的主路径，把过长的细节放到独立文档/网站。
- 文档必须与当前实现一致：
  - 命令示例应尽量来自真实 `--help` 输出或可复现的工作流
  - 重要术语（kam、kamfw、KamModuleLab、phase）要保持一致

### 构建文档站（KamWiki）

- 目录：`KamWiki/`
- 如需更新文档站内容，请在 PR 中描述如何本地预览与构建。


## 5. 翻译贡献（i18n）

- 相关入口：`src/i18n.rs`
- 原则：保持术语一致，避免同一概念多译。


## 6. AI 使用规范

- 本仓库接受 AI 辅助代码，但必须满足：
  - 可读、可维护
  - 遵循本仓库的 DEC 强制规范
  - PR 描述中说明关键设计与风险

补充说明请参考仓库内的 `AGENTS.md`
