---
status: "active"
date: "2026-01-01 02:00"
summary: "Clippy 修复进度总结与第二轮指令"
---

# Clippy 修复进度总结

## 第一轮修复结果

### ✅ 已完成
- **编译错误修复**：`src/cmds/tmpl/import.rs`, `src/cmds/tmpl/handler.rs` 等
- **C01 修复**：部分 `uninlined_format_args` 和 `items_after_statements`（`src/cmds/sign/**`, `src/cmds/tmpl/**`）
- **C02 修复**：部分 `missing_errors_doc`（多个文件）
- **C03 修复**：`too_many_lines` 重构（`src/cmds/secret/handler.rs`, `src/cmds/secret/index.rs`）
- **C04 修复**：部分 `needless_pass_by_value` 和 `must_use_candidate`（多个文件）

### ❌ 未完成
- **仍有 204 个 clippy 错误**

---

## 剩余错误分析（204 个）

### 主要错误类型
1. **`uninlined_format_args`** - 约 60+ 个
   - `src/utils.rs`（8 个）
   - `src/i18n.rs`（5 个）
   - `src/cmds/toml/handler.rs`（5 个）
   - `src/types/kam_toml/**`（8 个）
   - `src/rules/**`（6 个）
   - 其他文件

2. **`needless_pass_by_value`** - 约 12+ 个
   - `src/cmds/secret/handler.rs`（多个函数）

3. **`items_after_statements`** - 约 12+ 个
   - 多个文件

4. **`missing_errors_doc`** - 约 12+ 个
   - 多个 public `Result` 函数

5. **其他类型**：
   - `unnecessary_wraps` - 2 个（`src/cmds/secret/index.rs`）
   - `needless_borrow` - 2 个（`src/cmds/sign/handler.rs`）
   - `if_not_else` - 1 个（`src/cmds/sign/sigstore.rs`）
   - `must_use_candidate` - 约 9 个
   - `unwrap_used` - 若干
   - `case_sensitive_file_extension_comparisons` - 若干
   - `too_many_lines` - 1 个（`src/utils.rs:522` 行）
   - 其他小类型错误

---

## 第二轮修复指令

已创建：`.collab/outbox/LEAD__second-round-clippy-fix__20260101-0200.md`

**新 Agent 分工**：
- **C05**：剩余 Format/Style 修复（扩展 C01）
- **C06**：剩余 API 签名修复（扩展 C04）
- **C07**：剩余文档 + 其他 Lint 修复

**SR02 职责**：协调 C05~C07 合并，最终验证 clippy 全绿。

---

## 文件清理

已清理的过时文件：
- `D01__doc-quality-gates-sync__20251231-2256.md`（旧版本）
- `D01__doc-quality-gates-sync__20251231-2305.md`（旧版本）
- `LEAD__sr02-clippy-coordination__20250101-0000.md`（已执行完）

---

## 下一步

1. **立即**：C05~C07 开始第二轮修复
2. **等待交付**：预计 1-2 小时
3. **SR02 合并**：预计 30 分钟
4. **最终验证**：`cargo clippy` 必须全绿

---

**目标**：204 个错误 → 0 个错误
