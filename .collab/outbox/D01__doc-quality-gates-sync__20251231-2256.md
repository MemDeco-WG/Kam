---
agent_id: D01
order_id: D01-ORDER-DOC-GATE-0001
subject: "文档同步：clippy gate（fmt/clippy 必跑）"
date: 2025-12-31
---

# D01 文档同步：clippy gate（简洁版）

## 0) 依据

- Q01 审计报告：`.collab/outbox/Q01__audit__clippy-p0__20251231-2248.md`
- 质量门槛规范：`.collab/specs/SPEC__quality-gates__v1.md`

结论要点（引用 Q01 结论）：
- `cargo fmt --check` 未过、`cargo clippy ... -D warnings` 大面积失败可直接判定为 **P0 阻塞**。


## 1) README.zh-CN.md 变更

新增「✅ 质量门槛（必跑）」段落（放在贡献指南之前），包含必跑命令：

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

对应目的：对用户/贡献者明确“在修复前文档不撒谎”，并把 gate 写成可复制命令。


## 2) AGENTS.md 变更

在「4) 必跑验证（最小集）」中新增并置顶 Rust 基线 gate：

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

并明确：clippy 未过按门槛通常视为 P0 阻塞。


## 3) KamWiki 变更

更新：`KamWiki/docs/quality-gates.zh-CN.md`

新增「必跑 Gate（Rust：fmt / clippy）」段落，并写入同样的可复制命令：

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```


## 4) 本次修改的文件清单

- `README.zh-CN.md`
- `AGENTS.md`
- `KamWiki/docs/quality-gates.zh-CN.md`

