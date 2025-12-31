---
decision_id: "DEC-coding-philosophy"
date: "2025-12-31"
status: "accepted"
owner: "user"
---

# DEC：编码哲学与强制工程规范（必须遵守）

本决策用于约束后续所有 Agent/PR 的编码风格与工程质量，防止“看起来能跑但不可维护”的退化。

## 1) Anti-Reinventing the Wheel（反造轮子禁令）
- **优先标准库/成熟库**：能用标准库一行解决的，禁止手写循环/算法。
- **第三方标准（按语言生态）**：
  - 网络：优先 httpx（async）/requests（sync）
  - 数据模型：Pydantic v2
  - 日期时间：pendulum 或 datetime
  - CLI：typer 或 click
- **必须给出理由**：若坚持手写基础算法/逻辑，必须证明内置能力不适用。

> 注：本仓库主体为 Rust + Shell。上述“Python 第三方标准”作为跨项目通用原则保留；Rust 部分对应的等价规则见第 5 节。

## 2) Anti-Fallback Mandate（禁止隐式回退/静默失败）
- **宁愿当行 crash / 显式报错，也禁止返回“看起来正确”的默认值**。
- 禁止：
  - Shell：吞错 `|| true`、`2>/dev/null` 后继续假装成功（除非明确标注“非关键路径”）
  - Rust：`unwrap_or/unwrap_or_default`、用默认值掩盖错误、`_ => ...` 隐藏未覆盖分支
- 错误分支必须：
  - 明确 `Result<T, E>` 上抛，或
  - 在“逻辑不可达”处 `panic!/unreachable!`（并写清不可达原因）

## 3) Anti-Nested Try-Except / Anti-Superficial Fix
- 若出现嵌套 try/复杂错误分支，必须拆函数、降低复杂度。
- 禁止“敷衍修复”：不能通过额外分支掩盖当前报错而引入更大技术债。

## 4) Rust 工程强制规范
- **从克隆派转向引用派**：优先 `&str` / `&[T]`，必要时用 `Cow`。
- **禁止滥用 `.clone()` / `.to_string()`**：需要在 PR 中解释必要性。
- **错误处理**：禁止 `unwrap/expect`（除测试/不可达处）。优先 `thiserror`（库）/`anyhow`（应用）。
- **迭代器优先**：能用 iterator chain 就不要索引循环。
- **性能与内存**：可预估容量就 `with_capacity`，避免无意义分配。
- **工具链**：必须通过 `cargo fmt` 与 `cargo clippy`（不允许忽略警告）。
- **类型即状态机**：避免 `bool + Option` 表示状态，优先 enum。

## 5) Shell 工程强制规范（kamfw）
- **输出统一**：用户可见输出必须走 `.kamfwrc` 的 `print/ui_print`（禁止 echo）。
- **去重**：禁止复制粘贴式 fallback；必须封装（例如 `kam_print/kam_error/kam_abort`）。
- **shellcheck**：脚本头必须声明 `# shellcheck shell=ash`（设备端 BusyBox ash 语境）。

## 6) 审计要求（PR/Agent 交付必做）
每次交付必须回答：
1. 是否引入了隐式降级或静默失败？若有，为什么必要？
2. 是否出现重复逻辑（特别是输出/错误处理）？是否已抽象复用？
3. Rust 是否新增了 `.clone()`/`.to_string()`？是否可用引用替代？
4. 是否通过 `fmt/clippy`？

