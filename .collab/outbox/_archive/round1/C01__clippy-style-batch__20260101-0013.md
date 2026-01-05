# C01: Clippy 风格错误批量修复报告

**Agent ID**: C01
**任务**: Format/Style 批量修复专员
**交付时间**: 2026-01-01 00:13
**范围**: `src/cmds/sign/**` 和 `src/cmds/tmpl/**`

---

## 修复摘要

本次修复针对 clippy 的两种"机械型风格错误"进行批量清理：
- `clippy::uninlined_format_args` - 将 `format!("...", var)` 改为 `format!("...{var}")`
- `clippy::items_after_statements` - 将语句后的项（如 `use` 语句）移到语句之前

**修复原则**：
- ✅ 仅做机械型修复，不涉及大重构
- ✅ 禁止新增 `#[allow(clippy::...)]` 注解
- ✅ 禁止扩大 lint 压制范围

---

## 修改文件列表

### 1. `src/cmds/sign/handler.rs`
- 修复 `uninlined_format_args`: 7 处
  - 第 44 行: `format!("Failed to parse private key PEM with passphrase: {}", e)` → `format!("Failed to parse private key PEM with passphrase: {e}")`
  - 第 50 行: `format!("Failed to parse private key PEM: {}", orig_err)` → `format!("Failed to parse private key PEM: {orig_err}")`
  - 第 63 行: `format!("Failed to create signer: {}", e)` → `format!("Failed to create signer: {e}")`
  - 第 66 行: `format!("Failed to update signer: {}", e)` → `format!("Failed to update signer: {e}")`
  - 第 69 行: `format!("Failed to sign: {}", e)` → `format!("Failed to sign: {e}")`
  - 第 80 行: `format!("{}.cert.pem", filename)` → `format!("{filename}.cert.pem")`
  - 第 85 行: `format!("{}.sig", filename)` → `format!("{filename}.sig")`

### 2. `src/cmds/sign/sigstore.rs`
- 修复 `uninlined_format_args`: 3 处
  - 第 21 行: `format!("Failed to serialize payload: {}", e)` → `format!("Failed to serialize payload: {e}")`
  - 第 90 行: `format!("{}.sigstore.json", filename)` → `format!("{filename}.sigstore.json")`
  - 第 93 行: `format!("Failed to serialize bundle JSON: {}", e)` → `format!("Failed to serialize bundle JSON: {e}")`

### 3. `src/cmds/tmpl/export.rs`
- 修复 `uninlined_format_args`: 5 处
  - 第 32 行: `format!("Template '{}' not found in cache", template_name)` → `format!("Template '{template_name}' not found in cache")`
  - 第 73-76 行: `format!("Template '{}' exported to {}", template_name, ...)` → `format!("Template '{template_name}' exported to {}", ...)`
  - 第 83-86 行: 同上
  - 第 129 行: `format!("Template '{}' not found, skipping", template_name)` → `format!("Template '{template_name}' not found, skipping")`
- 修复 `items_after_statements`: 2 处
  - 第 119 行: 将 `use crate::utils::Utils;` 从循环内移到循环前（函数作用域内）
  - 第 185 行: 移除重复的 `use crate::utils::Utils;`（已在函数开头声明）

### 4. `src/cmds/tmpl/handler.rs`
- 修复 `items_after_statements`: 1 处
  - 第 47-49 行: 将 `use crate::utils::Utils;` 从 `TemplateCacheManager::remove_template(name)?;` 之后移到之前

### 5. `src/cmds/tmpl/import.rs`
- 修复 `uninlined_format_args`: 8 处
  - 第 48 行: `format!("{}.tar.gz", template_name)` → `format!("{template_name}.tar.gz")`
  - 第 83 行: `format!("Failed to open ZIP archive: {}", e)` → `format!("Failed to open ZIP archive: {e}")`
  - 第 96 行: `format!("Failed to read ZIP entry: {}", e)` → `format!("Failed to read ZIP entry: {e}")`
  - 第 153 行: `format!("Template '{}' already exists, skipping", top)` → `format!("Template '{top}' already exists, skipping")`
  - 第 163 行: `format!("Failed to read ZIP entry: {}", e)` → `format!("Failed to read ZIP entry: {e}")`
  - 第 198 行: `format!("Template '{}' imported", top)` → `format!("Template '{top}' imported")`
  - 第 65 行: `format!("Template '{}' imported successfully", template_name)` → `format!("Template '{template_name}' imported successfully")`
- 修复 `items_after_statements`: 4 处
  - 第 64 行: 将 `use crate::utils::Utils;` 从 `fs::copy(...)` 之后移到之前（`import_single_template` 函数内）
  - 第 116 行: 将 `use crate::utils::Utils;` 从 `if dest_path.exists() && !force` 之后移到之前（循环内）
  - 第 152 行: 将 `use crate::utils::Utils;` 从 `if dest_dir.exists() && !force` 之后移到之前（循环内）
  - 第 199 行: 移除重复的 `use crate::utils::Utils;`（已在循环开头声明）

---

## 修复统计

### Lint 类别统计
- **`uninlined_format_args`**: 约 23 处
- **`items_after_statements`**: 约 7 处
- **总计**: 约 30 处修复

### 文件统计
- 修改文件数: 5 个
- 修改行数: 约 30 行

---

## Clippy 验证结果

### 修复前（参考）
```bash
# 从 clippy 输出中统计到的相关错误提示行数
grep -c "uninlined_format_args\|items_after_statements" /tmp/clippy_before.txt
# 结果: 72 (包含帮助信息行)
```

### 修复后
```bash
# 在 sign 和 tmpl 目录中的相关错误数
cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1 | \
  grep -E "error.*uninlined_format_args|error.*items_after_statements" | \
  grep -E "src/cmds/(sign|tmpl)/" | wc -l
# 结果: 0 ✅
```

### 整体 clippy 状态
```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1 | \
  grep -E "error:" | wc -l
# 结果: 2 (其他非目标 lint 错误，不在本次修复范围内)
```

**结论**: 在 `src/cmds/sign/**` 和 `src/cmds/tmpl/**` 范围内，所有 `uninlined_format_args` 和 `items_after_statements` 错误已全部修复 ✅

---

## 验证命令

```bash
# 格式检查（必须通过）
cargo fmt --check

# Lint 检查（必须通过；将 warning 视为 error）
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 构建检查
cargo build --release
```

---

## 备注

1. 本次修复仅针对指定的两种 lint 规则，其他 clippy 错误不在本次修复范围内
2. 所有修复均为机械型修复，未涉及逻辑重构
3. 未新增任何 `#[allow(clippy::...)]` 注解
4. 修复后的代码已通过 `cargo fmt --check` 验证

---

**Agent C01 交付完成**
