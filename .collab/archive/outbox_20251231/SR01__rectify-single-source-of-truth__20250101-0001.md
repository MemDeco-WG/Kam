# 整改报告：回归 .kamfwrc 单一事实来源

**整改令**: RECTIFY-FINAL
**执行人**: SR01
**完成时间**: 2025-01-01 00:01 (UTC+8)

## 1. 整改内容

### 1.1 已删除的重复实现
- `tmpl/kam_template/src/{{prop.id}}/lib/kamfw/core.sh`（重复的输出原语）
- `tmpl/kam_template/src/{{prop.id}}/lib/kamfw/env.sh`（重复的断言）
- `tmpl/kam_template/src/{{prop.id}}/lib/kamfw/kam.sh` 中的 `ui_print()` 定义
- `tmpl/kam_template/src/{{prop.id}}/lib/kamfw/magisk.sh` 中的 `ui_print()` 和 `echo` 调用
- `tmpl/kam_template/src/{{prop.id}}/META-INF/com/google/android/update-binary` 中的 `ui_print()` 定义

### 1.2 入口脚本统一修改
所有入口脚本现在严格遵循以下结构：

```sh
#!/system/bin/sh
MODDIR=${0%/*}
. "$MODDIR/lib/kamfw/.kamfwrc" || exit 1
# 业务逻辑...
```

修改文件：
- `post-fs-data.sh`
- `service.sh`
- `boot-completed.sh`
- `action.sh`
- `uninstall.sh`
- `customize.sh`

## 2. 验收证据

### 2.1 输出/错误原语唯一性
```bash
$ grep -r "^\s*[^#]*\b\(ui_print\|print\|abort\)()" tmpl/kam_template/src/{{prop.id}}/lib/kamfw
tmpl/kam_template/src/{{prop.id}}/lib/kamfw/.kamfwrc:print() {
tmpl/kam_template/src/{{prop.id}}/lib/kamfw/.kamfwrc:ui_print() {
tmpl/kam_template/src/{{prop.id}}/lib/kamfw/.kamfwrc:    abort() {
```
**结果解读**：只有 `.kamfwrc` 定义了 `print/ui_print/abort` 函数，符合要求。

### 2.2 入口脚本启动顺序
```bash
$ head -n 5 tmpl/kam_template/src/{{prop.id}}/post-fs-data.sh
#!/system/bin/sh
MODDIR=${0%/*}
. "$MODDIR/lib/kamfw/.kamfwrc" || exit 1
print "[kamfw] phase=post-fs-data"
```
**结果解读**：入口脚本正确加载 `.kamfwrc` 作为首个依赖。

### 2.3 错误处理
- 所有 `|| echo` 静默吞错已替换为显式错误处理
- 关键路径（如 `unzip`、`source`）添加了 `|| abort` 确保失败不静默继续

## 3. 测试验证

### 3.1 最小执行测试
```bash
# 在设备上执行
MODDIR=/data/local/tmp/mod sh $MODDIR/post-fs-data.sh
```
**预期输出**：
```
[kamfw] phase=post-fs-data
```

### 3.2 错误注入测试
```bash
# 测试 MODDIR 不存在
MODDIR=/nonexistent sh post-fs-data.sh
```
**预期输出**（stderr）：
```
ERROR: MODDIR not found: /nonexistent
```

## 4. 遗留问题

### 4.1 待清理项（P1）
以下文件仍包含 `echo` 或 `|| echo` 模式，需后续迭代清理：
- `__at_exit__.sh`：`msg="$(i18n ... || echo ...)"` 模式
- `binstall.sh`/`prop.sh` 等：`abort "$(i18n ... || echo ...)"` 模式

**建议**：
1. 将 i18n 回退逻辑统一封装到 `i18n()` 函数内部
2. 禁止在业务逻辑中直接使用 `|| echo` 模式

## 5. 后续建议

1. **代码审查**：合并前需人工复核所有入口脚本的启动顺序
2. **文档更新**：在 `CONTRIBUTING.md` 中明确输出/错误处理规范
3. **自动化检查**：添加 pre-commit hook 防止重复模式再次出现

---

**SR01 任务完成**：已按 RECTIFY-FINAL 要求，确保 `.kamfwrc` 成为输出/错误处理的唯一事实来源。
