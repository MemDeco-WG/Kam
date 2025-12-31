---
agent_id: Q01
order_id: Q01-ORDER-0001
priority: P0
subject: "专项审计：kamfw 输出函数重复/启动流程不一致"
date: 2025-12-31
---

# Q01 越权直报：kamfw 启动流程与输出实现重复（P0）

## (A) 结论摘要：P0 拒收点列表（逐条）

1) **`ui_print()` 被多处定义（重复造轮子）**：
   - `.kamfwrc` 已定义 `ui_print()`（且声明其应为基础输出入口）。
   - `lib/kamfw/kam.sh` 再次定义 `ui_print()`（同名重复）。
   - `lib/kamfw/magisk.sh` 生成的 `update-binary` 内嵌 `ui_print(){ echo ... }`（又一份输出实现）。

2) **存在“复制粘贴式 fallback / 多处输出分支”**：
   - `base.sh`、`init_dirs.sh` 中出现 `command -v print` / `command -v ui_print` 的分支判断。
   - 这等价于“基础输出能力不确定/未保证启动顺序”，属于工程红线：输出实现散落、启动流程不一致。

3) **`.kamfwrc` 内部本身存在 Anti-Fallback 违规点（P0）**：
   - `set_perm()` 中：`chcon ... 2>/dev/null || true` 直接吞错继续跑。
   - 按 `.collab/decisions/DEC__coding-philosophy__20251231-1400.md` 的 **Anti-Fallback Mandate**：禁止静默失败/吞错继续跑（除非明确标注“非关键路径”并有理由）。

4) **启动顺序的“单一事实来源”未被硬性保证**：
   - 目前需要靠各处 `command -v` 来兜底，说明入口脚本并未强制做到“先 source `.kamfwrc`，再调用 print/ui_print/abort”。

> 裁决：以上任一条足以 P0 拒收；本次同时触发多条。


## (B) 证据

> 证据收集按指令要求：贴出命令与真实结果（可截断）。

### 证据 1：查找函数定义

命令：

```bash
rg -n "^print\(\)|^ui_print\(\)|^abort\(\)" tmpl/kam_template/src/{{prop.id}}/lib/kamfw
```

结果（截断）：

```text
tmpl/kam_template/src/{{prop.id}}/lib/kamfw/kam.sh
    16→ui_print() {

tmpl/kam_template/src/{{prop.id}}/lib/kamfw/magisk.sh
    34→ui_print() { echo "$1"; }
```

补充代码片段：

- `.kamfwrc`（基础函数来源）：

```sh
print() {
    if [ "${BOOTMODE:-true}" = "true" ]; then
        printf '%s\n' "$1"
    elif [ -e "/proc/self/fd/${OUTFD:-}" ]; then
        printf 'ui_print %s\nui_print\n' "$1" >&"$OUTFD"
    fi
}

ui_print() {
    print "$@"
}

if ! has_command "abort"; then
    abort() {
        print "ERROR: $1"
        exit 1
    }
fi
```

- `lib/kamfw/kam.sh`（重复定义 `ui_print`）：

```sh
ui_print() {
    print "$@"
}
```

- `lib/kamfw/magisk.sh`（生成的 update-binary 内嵌 `ui_print`，且用 echo）：

```sh
# echo before loading util_functions
ui_print() { echo "$1"; }
```

### 证据 2：查找重复 fallback

命令：

```bash
rg -n "command -v print|command -v ui_print" tmpl/kam_template/src/{{prop.id}}/lib/kamfw
```

结果（截断）：

```text
tmpl/kam_template/src/{{prop.id}}/lib/kamfw/base.sh
    9→  if command -v print >/dev/null 2>&1; then
    11→  elif command -v ui_print >/dev/null 2>&1; then

tmpl/kam_template/src/{{prop.id}}/lib/kamfw/init_dirs.sh
    12→    if command -v print >/dev/null 2>&1; then
    36→        if command -v print >/dev/null 2>&1; then
    50→        if command -v print >/dev/null 2>&1; then
```

这类分支等价于“输出能力不确定”，违反“单一事实来源”与“禁止复制粘贴式 fallback”。

### 证据 3：查找入口脚本是否 source .kamfwrc

> 按指令命令执行：

```bash
rg -n "\. .*lib/kamfw/\.kamfwrc" tmpl/kam_template/src/{{prop.id}}/*.sh
```

结果：

```text
# （无匹配）
```

说明：顶层入口脚本并未以统一方式显式 source `lib/kamfw/.kamfwrc`（至少不符合该 grep pattern），导致后续不得不在各处用 `command -v` 做兜底。


## (C) 整改建议（最小整改方案：方向与清单；由 SR01 执行）

> 目标：**只保留一个事实来源：`.kamfwrc`**；并且启动流程严格：入口脚本必须最先 source `.kamfwrc`。

### C1. 必须删除/回滚/改为复用的清单（P0）

1) `tmpl/kam_template/src/{{prop.id}}/lib/kamfw/kam.sh`
   - **动作**：删除其中的 `ui_print()` 定义。
   - **理由**：`.kamfwrc` 已定义 `ui_print()`；重复定义触发 P0。

2) `tmpl/kam_template/src/{{prop.id}}/lib/kamfw/magisk.sh`
   - **动作**：停止在生成的 `update-binary` 里内嵌 `ui_print(){ echo ... }`。
   - **替代方向**：
     - 生成脚本应尽可能**先 source Magisk 官方 `util_functions.sh`** 并使用其 `ui_print`；
     - 或者明确约定：`update-binary` 里只允许一种输出实现，并在顶部立即导入 `.kamfwrc`（若路径与执行环境允许）。
   - **理由**：当前是第三份输出实现，且直接 `echo`，违反输出统一。

3) `tmpl/kam_template/src/{{prop.id}}/lib/kamfw/base.sh`
   - **动作**：移除 `command -v print/ui_print` 的 fallback 分支。
   - **前置**：保证调用者已经 source `.kamfwrc`。

4) `tmpl/kam_template/src/{{prop.id}}/lib/kamfw/init_dirs.sh`
   - **动作**：同上，移除所有 `command -v print` 分支。

5) `tmpl/kam_template/src/{{prop.id}}/lib/kamfw/.kamfwrc`
   - **动作（P0）**：修正 `chcon ... 2>/dev/null || true` 的吞错行为。
   - **建议方向**：
     - 要么改为失败即 abort；
     - 要么显式标注“非关键路径”，并打印一次可见警告（仍不建议 `|| true` 静默）。

### C2. 启动流程统一（单一入口）

- 由 SR01 定义并强制一个“入口加载器”策略：
  - 顶层入口脚本（例如 `customize.sh`/`service.sh`/`post-fs-data.sh`/`uninstall.sh` 等）必须在最顶部：
    1) 计算 `MODDIR`
    2) `.` source `lib/kamfw/.kamfwrc`
    3) 然后才能调用 `print/ui_print/abort` 以及 `import ...`

- 并新增一个最小“自检”约束（方向）：
  - 在 `.kamfwrc` 或入口处断言 `print/ui_print/abort` 已定义，否则直接 abort（禁止 fallback）。

### C3. SR01 执行步骤（建议顺序）

1) 先改入口：确保所有顶层脚本最先 source `.kamfwrc`（一次性解决“输出能力不确定”的根因）。
2) 删除 `kam.sh` 的 `ui_print` 重复定义。
3) 逐个清理 `base.sh`、`init_dirs.sh` 的 `command -v` fallback 分支。
4) 修复 `.kamfwrc` 里 `chcon ... || true` 的吞错（按 DEC 要求）。
5) 最后处理 `magisk.sh` 的 `update-binary` 生成逻辑，确保输出实现单一且可预测。


## 附：关联强制规范

- `.collab/decisions/DEC__coding-philosophy__20251231-1400.md`
  - Anti-Reinventing the Wheel（反造轮子禁令）
  - Anti-Fallback Mandate（禁止隐式回退/静默失败）
  - Shell 输出统一（必须走 `.kamfwrc` 的 print/ui_print）
