---
spec_id: "SPEC-quality-gates-v1"
date: "2025-12-31"
status: "active"
authority: "Lead (with user rules)"
inspector: "Q01"
---

# 质量门槛（Quality Gates）v1

本规范定义项目的硬性质量门槛。任何 Agent/PR 交付若触发 **P0 红线**，默认直接拒收并要求整改；P1 限期整改；P2 记录在案。

> 权威来源：`.collab/decisions/DEC__coding-philosophy__20251231-1400.md`

## 0. 角色与越权流程（Q01 / Lead）

- **Q01（代码质量检察员）**：可越权直接向 Lead 报告 P0/P1/P2 问题。
- **Lead（裁决）**：对是否拒收、整改期限、整改负责人做最终决定。
- **执行 Agent（如 SR01/H01/B00/D01）**：负责实际修复。

Q01 报告格式：`.collab/outbox/Q01__audit__YYYYMMDD-HHMM.md`，必须包含 YAML 头 + (A)(B)(C)。

## 1. P0 红线（触发即拒收）

### P0-1 隐式回退 / 静默失败（Anti-Fallback Mandate）
**定义**：代码在错误/缺失输入情况下继续运行，并返回“看起来正确”的结果，导致 false positives。

**判定标准（满足任一即 P0）**：
- Shell：
  - `|| true`、`2>/dev/null` 吞错后继续跑（除非明确标注“非关键路径”且有原因）
  - 关键变量缺失仅 warning 不退出（如 MODDIR/KAM_HOME/ZIPFILE 等关键路径）
  - 安装/运行阶段出现“失败但继续”的行为（如 unzip/cp/install 失败后继续）
- Rust（若存在）：
  - `unwrap_or/unwrap_or_default` 用默认值掩盖错误
  - `match _ => ...` 用 `_` 隐藏未处理状态

**整改方向**：
- 必须显式 `abort/exit != 0`（shell）或 `Result<T,E>` 上抛（rust）。

### P0-2 重复逻辑 / 复制粘贴式 fallback（Anti-Duplication）
**定义**：同一类逻辑（输出/错误处理/env assert/路径推导）被复制粘贴在多个文件/函数中。

**判定标准**：
- 出现多处类似：
  - `if command -v print ... else printf ... fi`
  - `if [ -z "$MODDIR" ]; then ... fi` 的重复实现
  - `KAM_HOME==MODDIR` 校验多处散落

**整改方向**：
- 抽象为唯一权威函数（例如 `kam_print/kam_error/kam_abort/kam_env_assert`），全仓库复用。

### P0-3 输出契约不一致 / 用户可见输出违规
**定义**：用户可见输出没有走统一输出通道，导致 Magisk/KSU/安装器环境不可见。

**判定标准**：
- 模块脚本、shim、kamfw 相关脚本中：
  - 使用 `echo` 输出用户提示/错误（可用 `print/ui_print/kam_print` 的场景下）
  - 输出通道不一致，导致部分环境无输出

**整改方向**：
- 统一输出原语；禁止 echo 作 UI 输出。

### P0-4 “敷衍修复”掩盖结构性问题（Anti-Superficial Fix）
**定义**：通过添加更多分支/吞错/默认值让错误消失，而不是解决根因。

**判定标准**：
- 修复只让错误不再出现，但没有解释根因、没有新增测试/验收门槛
- 通过扩大 try/if/兼容分支来绕过而非拆分/抽象

**整改方向**：
- 必须结构性重构：拆函数、抽象复用、增加验收命令或测试。

## 2. P1 问题（限期整改）

### P1-1 不一致命名/约定（左右脑互搏）
**判定标准**：
- 同一概念多套变量/路径（例如 TMPDIR/KAM_TMPDIR/KAM_TMP_HOME 混用无规范）
- 不同入口脚本 import 顺序/初始化顺序不一致

**整改方向**：
- 输出一份规范并统一替换；入口脚本采用同一最小 wrapper。

### P1-2 文档与代码不一致
**判定标准**：
- README/文档声称存在某目录/命令/流程，但仓库中不存在或行为不同

**整改方向**：
- 以代码为准修正文档；或补齐实现，并在变更中说明。

## 3. P2 问题（记录在案，择期清理）
- 注释不清、命名不佳、可读性一般
- 非关键路径的小性能问题
- 可替换为更惯用写法但不影响正确性

## 4. 快速自检清单（交付前必须做）

### Shell 自检
- [ ] 关键路径无 `|| true` 静默吞错
- [ ] 用户可见输出不使用 `echo`
- [ ] 输出/错误处理通过单一原语封装
- [ ] 入口脚本结构统一（source/初始化/phase 调度一致）

### Rust 自检（若存在）
- [ ] 无 `unwrap/expect`（测试/不可达除外）
- [ ] 无 `unwrap_or*` 用默认值掩盖错误
- [ ] `match` 显式穷举变体

## 5. 触发后的处理
- P0：Q01 直接报告，Lead 默认拒收并指定整改负责人/期限
- P1：Lead 指定整改期限（通常 24~72h）
- P2：记录在案，可并入后续重构里程碑
