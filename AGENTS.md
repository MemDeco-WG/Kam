# Kam 项目协作手册（AGENTS）

本文件是主项目级的协作与质量门槛手册，约束 Agent/PR 的交付形式、验证命令、审计与验收流程。

> 权威规范：
> - 质量门槛：`.collab/specs/SPEC__quality-gates__v1.md`
> - 编码哲学：`.collab/decisions/DEC__coding-philosophy__20251231-1400.md`
>
> 若本文件与上述规范冲突，以规范为准。


## 1) 项目角色说明：SR01/H01/B00/Q01/D01

所有协作交付统一提交到 `.collab/outbox/`，并遵守命名规范。

### 角色与职责

- **SR01（实施修复 / 集成交付）**
  - 职责：按 Lead 裁决落地代码修复、重构与对齐（Rust/Shell/模板/脚本）。
  - 交付：修复性报告、变更摘要、必要的最小验证结果。

- **H01（命令帮助矩阵 / CLI 一致性）**
  - 职责：梳理 `kam --help` 与各子命令 `--help` 的输出一致性、缺失项、文案问题；维护 help 矩阵。
  - 交付命名：`.collab/outbox/H01__<topic>__YYYYMMDD-HHMM.md`

- **B00（构建与发布 / 工具链）**
  - 职责：构建流程、打包、签名/验证、CI 与发布相关改动；维护可复现构建。
  - 交付命名：`.collab/outbox/B00__<topic>__YYYYMMDD-HHMM.md`

- **Q01（代码质量检察 / Linus Mode）**
  - 职责：执行质量门槛审计（P0/P1/P2）；发现 P0 可越权直报 Lead。
  - 报告格式（强制）：`.collab/outbox/Q01__audit__<topic>__YYYYMMDD-HHMM.md`
  - 内容必须包含：YAML 头 + (A)结论摘要 + (B)证据 + (C)最小整改建议。

- **D01（文档工程 / 入口叙事）**
  - 职责：维护 README、贡献指南、协作手册等文档，使其与仓库现状一致。
  - 交付命名：`.collab/outbox/D01__<topic>__YYYYMMDD-HHMM.md`

### 交付物命名规范

- 统一格式：`<AgentID>__<snake-topic>__YYYYMMDD-HHMM.md`
- 示例：`D01__agents-md-main__20251231-1440.md`


## 2) 质量门槛：引用并链接

- 质量门槛（Quality Gates）v1：
  - `.collab/specs/SPEC__quality-gates__v1.md`

- 编码哲学与强制工程规范：
  - `.collab/decisions/DEC__coding-philosophy__20251231-1400.md`


## 3) 越权机制：Q01 的 P0 直报流程与 Lead 裁决流程

当发现 **P0 红线**（例如：隐式回退/静默失败、重复造轮子、输出契约不一致等），Q01 可以直接提交审计报告并“越权直报” Lead。

- Q01 提交：`.collab/outbox/Q01__audit__...md`
- Lead 裁决：
  - 是否拒收
  - 指定整改负责人（通常 SR01）与期限
  - 明确验收命令/证据要求


## 4) 必跑验证（最小集）

> 交付前至少跑完本节命令，并在 outbox 报告中记录结果（成功/失败 + 关键输出）。

### 4.1 Rust 基线：fmt / clippy（必跑 Gate）

```bash
# 格式检查（必须通过）
cargo fmt --check

# Lint（必须通过；将 warning 视为 error）
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

> 说明：此门槛为 **P0 级**。若 `clippy` 未通过（例如出现大量 warning/error），或通过添加 `#[allow(clippy::...)]` 注解来“作弊”以压制告警，均视为 P0 阻塞。必须先将 lint 清理干净，再进行功能开发/合并。

### 4.2 cargo build --release（主项目）

```bash
cargo build --release
```

### 4.3 kam --help + 子命令 --help

```bash
./target/release/kam --help
./target/release/kam <subcommand> --help
```

建议至少覆盖：`init`、`build`、`tmpl`、`export`、`config`、`toml`、`sign`、`verify`、`check`、`repo`。

### 4.4 kam init . --tmpl（P0 gate）

```bash
./target/release/kam init . --tmpl --force
```

- 当前该命令为 **P0 gate**：若失败，必须在交付中说明失败原因与影响范围，并由 Lead 决定是否拒收/延期。


## 5) 提交/验收流程：outbox → Q01 审计 → Lead 验收 → inbox

1. **执行 Agent** 输出交付物到：`.collab/outbox/`
2. **Q01 审计**：
   - 通过：给出“可验收”结论（或无 P0/P1）
   - 不通过：提交 `Q01__audit__...` 并标注 P0/P1/P2
3. **Lead 验收**：
   - 根据 Q01 结论与必跑验证决定是否合入
4. **inbox 归档（按 Lead 流程）**：
   - 通过的交付可从 outbox 归档/转入 `.collab/inbox/`（如项目流程要求）
