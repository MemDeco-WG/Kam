---
decision_id: "DEC-kam-home-is-moddir"
date: "2025-12-31"
status: "accepted"
---

# DEC：MODDIR 即 HOMEDIR

## 结论
- `KAM_HOME="$MODDIR"`
- 同时导出 `HOME="$MODDIR"`（兼容部分工具/库默认读取 HOME）

## 目录布局（直接位于 $MODDIR 下）
- `.config/`
- `.local/bin/`
- `.local/lib/`
- `.cache/`
- `.state/`
- `.log/`
- `tmp/`

## 约束
- 不允许使用 `$MODDIR/home` 作为 HOME 根（避免分叉）。
- Rust 与 Shell 必须对齐该约定。
