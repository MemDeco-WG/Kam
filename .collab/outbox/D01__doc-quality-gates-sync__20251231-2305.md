# D01-ORDER-DOC-GATE-0001: 质量门槛与开发流程文档同步

## 1. 更新内容概览

### 1.1 新增 P0 Gate 要求
- 添加了 `cargo fmt --check` 和 `cargo clippy` 作为强制门禁
- 禁止通过 `#[allow]` 绕过门禁（需 Lead 书面批准）
- 明确贡献者本地必跑命令

### 1.2 更新文件
1. `README.zh-CN.md` - 添加质量门槛章节和本地开发要求
2. `AGENTS.md` - 更新质量门槛和必跑验证部分
3. `KamWiki/docs/quality-gates.zh-CN.md` - 添加门禁详细说明

## 2. 文件变更详情

### 2.1 README.zh-CN.md 更新

在「开发工作流」章节后添加：

```markdown
## 🛠️ 质量门槛（Quality Gates）

Kam 项目设有严格的质量门槛，所有代码在合并前必须通过以下检查：

### 必跑门禁（P0 Gate）

```bash
# 1. 代码格式化检查
cargo fmt --check

# 2. Clippy 静态分析（零警告）
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 3. 构建检查
cargo build --release
```

### 重要规则
- **禁止** 使用 `#[allow(...)]` 绕过 Clippy 警告（需 Lead 明确批准）
- 所有测试必须通过
- 代码覆盖率不能低于主分支

### 本地开发流程
1. 开发前拉取最新代码：`git pull --rebase`
2. 运行 `cargo check` 进行快速验证
3. 提交前运行完整门禁：`./scripts/check-gates.sh`
4. 确保所有测试通过：`cargo test --workspace`
```

### 2.2 AGENTS.md 更新

在「4) 必跑验证（最小集）」章节更新：

```markdown
### 4.4 代码质量门禁（P0 Gate）

所有 Agent 在提交前必须运行以下命令并确保通过：

```bash
# 代码格式化检查
cargo fmt --check

# Clippy 静态分析（零容忍）
# 注意：不允许添加新的 `#[allow]` 属性，除非获得 Lead 明确批准
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 完整构建
cargo build --release

# 运行测试
cargo test --workspace
```

**违规处理**：
- 任何门禁失败都会导致 PR 被标记为 "do not merge"
- 发现新警告需要修复，不允许压制
- 特殊情况下需要豁免必须记录原因并获得 Lead 批准
```

### 2.3 KamWiki/docs/quality-gates.zh-CN.md 更新

在「P0 红线摘要」后添加：

```markdown
## 开发流程强制要求

### 1. 本地开发必跑命令

```bash
# 1. 代码格式化
cargo fmt

# 2. 静态检查（零警告）
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 3. 运行测试
cargo test --workspace

# 4. 完整构建
cargo build --release
```

### 2. 提交规范
- 每个提交必须通过 `cargo fmt` 和 `cargo clippy`
- 提交信息需清晰描述变更内容
- 关联 issue 编号（如：`fix #123`）

### 3. 代码审查要求
- 审查时需验证门禁是否通过
- 关注代码质量而不仅是功能
- 确保没有新增的 `#[allow]` 属性
```

## 3. 实施说明

1. 上述变更需要同步到所有相关文档
2. 更新 CI/CD 流水线以强制执行这些门禁
3. 通知所有贡献者新的开发流程要求
4. 在团队会议中强调这些要求的重要性

## 4. 后续步骤

- [ ] 合并文档更新
- [ ] 更新 CI/CD 配置
- [ ] 通知团队成员
- [ ] 在下次站会中同步信息
