---
directive_to: "C05, C06, C07, SR02"
date: "2025-01-01"
priority: "P0"
status: "active"
context: "Clippy 第二轮修复（剩余 204 个错误）"
---

# Clippy 第二轮修复指令

## 当前状态

**第一轮修复结果**（SR02 报告）：
- ✅ 编译错误已修复
- ✅ C01~C04 部分修复已合并
- ❌ 仍有 **204 个 clippy 错误**需要修复

**主要错误类型**（基于 clippy 输出分析）：
1. `needless_pass_by_value` - 约 12+ 个（主要在 `src/cmds/secret/handler.rs`）
2. `uninlined_format_args` - 约 60+ 个（分散在多个文件）
3. `items_after_statements` - 约 12+ 个
4. `missing_errors_doc` - 约 12+ 个
5. `unnecessary_wraps` - 2 个（`src/cmds/secret/index.rs`）
6. `needless_borrow` - 2 个（`src/cmds/sign/handler.rs`）
7. `if_not_else` - 1 个（`src/cmds/sign/sigstore.rs`）
8. `must_use_candidate` - 约 9 个
9. 其他类型错误（`unwrap_used`, `case_sensitive_file_extension_comparisons`, 等）

**错误最多的文件**：
- `src/utils.rs` - 8 个
- `src/i18n.rs` - 5 个
- `src/cmds/toml/handler.rs` - 5 个
- `src/types/kam_toml/sections/prop.rs` - 4 个
- `src/types/kam_toml.rs` - 4 个
- `src/cmds/tmpl/import.rs` - 4 个

---

## 第二轮修复 Agent 分工

### C05：剩余 Format/Style 修复（扩展 C01 工作）

**负责**：
- 修复剩余的 `uninlined_format_args`（约 60+ 个）
- 修复剩余的 `items_after_statements`（约 12+ 个）

**文件范围**（优先）：
- `src/utils.rs`（8 个错误）
- `src/i18n.rs`（5 个错误）
- `src/cmds/toml/handler.rs`（5 个错误）
- `src/types/kam_toml/**`（8 个错误）
- `src/rules/**`（6 个错误）
- 其他剩余文件

**规则**：
- 仅做机械型修复，不涉及大重构
- 禁止新增 `#[allow(clippy::...)]` 注解

**交付物**：`.collab/outbox/C05__clippy-style-remaining__YYYYMMDD-HHMM.md`

---

### C06：剩余 API 签名修复（扩展 C04 工作）

**负责**：
- 修复剩余的 `needless_pass_by_value`（约 12+ 个）
- 修复剩余的 `must_use_candidate`（约 9 个）
- 修复 `unnecessary_wraps`（2 个）
- 修复 `needless_borrow`（2 个）
- 修复 `if_not_else`（1 个）

**文件范围**（优先）：
- `src/cmds/secret/handler.rs`（多个 `needless_pass_by_value`）
- `src/cmds/secret/index.rs`（`unnecessary_wraps`）
- `src/cmds/sign/handler.rs`（`needless_borrow`）
- `src/cmds/sign/sigstore.rs`（`if_not_else`）
- 其他剩余文件

**规则**：
- 修改签名必须同时修改调用点
- 不允许 `#[allow]` 压制

**交付物**：`.collab/outbox/C06__clippy-api-remaining__YYYYMMDD-HHMM.md`

---

### C07：剩余文档 + 其他 Lint 修复

**负责**：
- 修复剩余的 `missing_errors_doc`（约 12+ 个）
- 修复其他类型错误：
  - `unwrap_used`（如存在）
  - `case_sensitive_file_extension_comparisons`（如存在）
  - 其他小类型错误

**文件范围**：
- 所有 public `Result` 函数（补充 `# Errors` 文档）
- 其他有 lint 错误的文件

**规则**：
- 只补充最小必要文档块
- 不得改动逻辑（除非为了消除 unwrap 等硬问题）

**交付物**：`.collab/outbox/C07__clippy-docs-others__YYYYMMDD-HHMM.md`

---

## SR02 协调职责（第二轮）

1. **等待 C05~C07 交付**
2. **按顺序合并**：
   - C07（文档，几乎无冲突）
   - C05（格式，低冲突）
   - C06（API 签名，可能冲突）
3. **最终验证**：
   - `cargo fmt --check`
   - `cargo clippy --workspace --all-targets --all-features -- -D warnings`（必须全绿）
   - `cargo build --release`
4. **提交合并报告**：`.collab/outbox/SR02__clippy-round2-merge__YYYYMMDD-HHMM.md`

---

## 禁止事项

- ❌ 禁止使用 `#[allow(clippy::...)]` 压制 lint
- ❌ 禁止"为了通过 clippy"而破坏代码结构
- ❌ 禁止合并未经验证的 patch

---

## 时间线

- **立即**：C05~C07 开始修复
- **预计 1-2 小时**：C05~C07 交付
- **预计 30 分钟**：SR02 合并 + 验证

---

**Lead 指令**：立即开始第二轮修复，目标：clippy 全绿。
