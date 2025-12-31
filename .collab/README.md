# .collab 协作目录（统一约定）

本目录用于多 Agent 协作交换信息。

## 目录结构
- `00_index.md`：Lead 维护的索引与关键决策（唯一真相源）
- `inbox/`：Lead 复制进来的“已确认可引用”产物（只读）
- `outbox/`：各 Agent 的交付物（只写这里）
- `decisions/`：关键决策记录（Lead 写）
- `specs/`：冻结的规范（Lead 汇总写）
- `plans/`：实施计划/里程碑/文件级改动清单（Lead 汇总写）

## 交付物格式（强制）
每个 Agent 的交付物必须是一个 Markdown 文件，且包含 YAML 头：

```yaml
---
agent_id: "A01"
role: "..."
topic: "..."
status: draft|ready|needs_feedback
provides: ["..."]
depends_on: [".collab/inbox/..."]
questions: ["..."]
files_touched_suggestion: ["path1", "path2"]
---
```

并包含三段：
- (A) 结论摘要（5~10 行）
- (B) 证据与引用（文件路径/函数名/片段）
- (C) 可执行变更清单（文件路径 -> 修改点 -> 原因 -> 风险）

## 命名规范
`.collab/outbox/<AGENT_ID>__<TOPIC>__YYYYMMDD-HHMM.md`

例：`.collab/outbox/A02__shell-import-graph__20251231-1235.md`
