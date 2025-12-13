# Shell脚本检查报告

## 检查概览

- **检查工具**: shellcheck
- **检查时间**: $(date)
- **检查文件数**: 74个shell脚本文件
- **发现问题总数**: 602个

## 问题类型统计

| 问题代码 | 数量 | 严重程度 | 说明 |
|---------|------|---------|------|
| SC2086 | 509 | info | 变量未加引号，可能导致单词分割和路径扩展 |
| SC1091 | 249 | info | 无法跟踪source的文件（需要-x参数） |
| SC2034 | 72 | warning | 变量定义但未使用 |
| SC2166 | 40 | warning | 使用-a/-o而非&&/|| |
| SC2059 | 24 | info | printf格式字符串中使用变量 |
| SC2329 | 22 | info | 函数定义但从未调用 |
| SC2164 | 17 | warning | cd命令未检查返回值 |
| SC2181 | 15 | warning | 检查命令失败但未处理 |
| SC2155 | 14 | warning | 声明和赋值应分开以避免掩盖返回值 |
| SC2046 | 14 | warning | 命令替换未加引号 |
| SC2148 | 8 | error | 缺少shebang或shell指令 |
| SC2006 | 8 | style | 使用反引号而非$() |

## 问题最多的文件

1. **tmpl/ak3_template/src/AnyKernel3/tools/ak3-core.sh** - 253个问题
2. **tmpl/kam_template/src/{{prop.id}}/lib/mmt-extended.sh** - 112个问题
3. **tmpl/kam_template/src/{{prop.id}}/lib/common_func.sh** - 15个问题
4. **tmpl/ak3_template/src/AnyKernel3/anykernel.sh** - 9个问题
5. **hooks/pre-build/8000.sync_hooks.sh** - 7个问题

## 主要问题分类

### 1. 变量引用问题 (SC2086) - 509个
最常见的问题，变量未加引号可能导致：
- 单词分割（word splitting）
- 路径扩展（pathname expansion）

**示例**:
```bash
# 错误
cd $KAM_HOOKS_SRC && git pull

# 正确
cd "$KAM_HOOKS_SRC" && git pull
```

### 2. Source文件跟踪问题 (SC1091) - 249个
shellcheck无法跟踪source的文件，这通常需要-x参数来解析。

**示例**:
```bash
. "$KAM_HOOKS_ROOT/lib/utils.sh"
```

### 3. 未使用的变量 (SC2034) - 72个
变量被定义但从未使用，可能是：
- 遗留代码
- 导出给外部使用（应export）
- 实际需要但shellcheck未检测到

### 4. 逻辑运算符问题 (SC2166) - 40个
使用`-a`和`-o`而非`&&`和`||`，在某些shell中可能有问题。

**示例**:
```bash
# 不推荐
[ "$NEWVALUE" = "$CURVALUE" -a ! "$FORCE" ]

# 推荐
[ "$NEWVALUE" = "$CURVALUE" ] && [ ! "$FORCE" ]
```

### 5. printf格式问题 (SC2059) - 24个
在printf格式字符串中直接使用变量，应使用%s占位符。

**示例**:
```bash
# 不推荐
printf "${BLUE}KAM variables:${NC}\n"

# 推荐
printf '%sKAM variables:%s\n' "$BLUE" "$NC"
```

## 建议修复优先级

### 高优先级（安全相关）
1. **SC2086** - 变量引用问题，可能导致安全漏洞或意外行为
2. **SC2164** - cd命令失败未处理，可能导致后续命令在错误目录执行
3. **SC2181** - 命令失败未处理

### 中优先级（代码质量）
1. **SC2166** - 逻辑运算符问题
2. **SC2155** - 声明和赋值分离
3. **SC2046** - 命令替换引号问题

### 低优先级（信息性）
1. **SC1091** - Source文件跟踪（通常可忽略）
2. **SC2034** - 未使用变量（需人工确认）
3. **SC2329** - 未调用函数（可能是回调函数）

## 修复建议

### 批量修复SC2086问题
可以使用以下命令查找所有需要修复的位置：
```bash
grep -n "SC2086" /tmp/shellcheck_all.log
```

### 修复示例

**hooks/pre-build/8000.sync_hooks.sh**:
```bash
# 修复前
cd $KAM_HOOKS_SRC && git pull origin main
cd -

# 修复后
cd "$KAM_HOOKS_SRC" && git pull origin main
cd - || exit
```

**hooks/pre-build/3000.BUILD_CRATES.sh**:
```bash
# 修复前
build_multi_arch $(detect_build_tool)

# 修复后
build_multi_arch "$(detect_build_tool)"
```

## 检查命令

重新运行检查：
```bash
find . -name "*.sh" -type f -exec shellcheck {} \;
```

查看详细报告：
```bash
cat /tmp/shellcheck_all.log
```

## 注意事项

1. 某些警告可能是误报，特别是：
   - SC1091（source文件跟踪）- 需要-x参数
   - SC2034（未使用变量）- 可能被外部使用
   - SC2329（未调用函数）- 可能是回调或条件调用

2. 模板文件中的问题（tmpl目录）会在生成时被处理，可能不需要立即修复。

3. 某些脚本可能是从其他项目复制而来，保持兼容性可能比修复所有警告更重要。
