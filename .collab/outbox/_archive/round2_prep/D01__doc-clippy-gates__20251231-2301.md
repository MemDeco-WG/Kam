---
agent_id: D01
order_id: D01-ORDER-DOC-GATES-0002
subject: "文档同步：P0 Gate（fmt + clippy -D warnings 必须全绿；禁止 allow 作弊）"
date: 2025-12-31
---

# D01 文档同步：clippy gates（P0 强化版）

## 目标

将以下门槛以“简洁、可执行、只写事实”的方式同步到：
- `README.zh-CN.md`
- `AGENTS.md`
- `KamWiki`（当前为 `KamWiki/docs/quality-gates.zh-CN.md`）

必须表达清楚两点：
1) **P0 Gate：`cargo fmt --check` + `cargo clippy ... -- -D warnings` 必须全绿**
2) **禁止通过 `#[allow(clippy::...)]` 作弊来压制告警**（必须修根因）

依据：
- `.collab/specs/SPEC__quality-gates__v1.md`
- `.collab/outbox/Q01__audit__clippy-p0__20251231-2248.md`


## 变更摘要（按文件）

### 1) README.zh-CN.md

位置：`✅ 质量门槛（必跑）` 章节

- 将说明文案升级为明确 **P0 Gate**
- 增加“禁止 allow 作弊”与“Q01 可判定 P0 并越权直报”的表述

关键段落：

```md
> 说明：**P0 Gate**。若 `clippy` 未通过，或试图用 `#[allow(clippy::...)]` 压制告警，Q01 可直接判定为 P0 阻塞并越权直报。
```

### 2) AGENTS.md

位置：`4.1 Rust 基线：fmt / clippy（必跑 Gate）`

- 将该 gate 明确标注为 **P0 级**
- 明确写出：添加 `#[allow(clippy::...)]` 压制告警视为“作弊”，同样 P0 阻塞

关键段落：

```md
> 说明：此门槛为 **P0 级**。若 `clippy` 未通过（例如出现大量 warning/error），或通过添加 `#[allow(clippy::...)]` 注解来“作弊”以压制告警，均视为 P0 阻塞。
```

### 3) KamWiki

文件：`KamWiki/docs/quality-gates.zh-CN.md`

- 在「必跑 Gate（Rust：fmt / clippy）」段落中补充两条 bullet：
  - clippy 不通过 = P0 阻塞
  - 禁止 `#[allow(clippy::...)]` 作弊，必须让 `-D warnings` 全绿

关键段落：

```md
- 若 `clippy` 未通过，按质量门槛视为 **P0 阻塞**。
- **禁止通过 `#[allow(clippy::...)]` 作弊**来压制告警；必须修正根因并让 `-D warnings` 全绿。
```


## 本次修改文件清单

- `README.zh-CN.md`
- `AGENTS.md`
- `KamWiki/docs/quality-gates.zh-CN.md`
