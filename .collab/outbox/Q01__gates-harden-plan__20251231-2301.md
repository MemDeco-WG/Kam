---
audit_id: Q01-ORDER-GATES-HARDEN-0003
created_at: "2025-12-31T23:01:45+08:00"
auditor: Q01
subject: "质量门槛硬化计划（v2）与强制 Gate 套件"
status: "提议（待 Lead 批准并写入 SPEC）"
---

## (A) 提议：升级质量门槛（Quality Gates v2）

为根除“敷衍修复”与“技术债扩散”，我提议将以下 Gate 套件写入 `.collab/specs/SPEC__quality-gates__v2.md`，作为所有代码提交的硬性验收标准。

**核心原则**：
- **P0 门槛**：必须 100% 通过，任何失败都会导致 **CI 阻塞与代码拒收**。
- **P1 门-槛**：建议在 CI 中运行并告警，合并前需人工复核。
- **P2 门槛**：用于本地开发自检与未来兼容性评估。

## (B) 强制 Gate 套件详解

### P0：基础一致性与正确性（必须全绿）

#### 1. `cargo fmt --check`
- **目的**：确保所有代码遵循统一的、由工具强制的格式规范。
- **抓住什么**：不一致的缩进、换行、间距等“破窗效应”代码。
- **常见失败**：IDE 自动格式化配置不当；手动调整对齐破坏了 `rustfmt` 规则。
- **拒收条件**：任何 `diff` 输出。

#### 2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- **目的**：静态分析代码，捕获常见错误、非惯用法、性能问题与潜在 bug。
- **抓住什么**：`unwrap()` 滥用、冗余闭包、可简化的 `if-else`、未使用的变量、文档缺失等。
- **常见失败**：代码风格不统一；错误处理不当；引入技术债。
- **拒收条件**：任何 `error:` 输出。

#### 3. `cargo test --workspace --all-targets --all-features`
- **目的**：确保所有单元测试、集成测试与文档测试通过。
- **抓住什么**：逻辑错误、边界条件失败、回归性 bug。
- **常见失败**：代码变更破坏了既有功能；测试用例未更新。
- **拒收条件**：任何 `FAILED` 的测试用例。

#### 4. `RUSTFLAGS="-D warnings -D unsafe_code" cargo build --workspace --all-targets --all-features`
- **目的**：以最严苛的编译选项构建项目，禁止任何 `unsafe` 代码块（除非特批）。
- **抓住什么**：
  - 编译器自身的告警（`clippy` 未覆盖的）。
  - **未被批准的 `unsafe` 代码**：这是 P0 级的安全红线。
- **常见失败**：引入了含 `unsafe` 的依赖；为局部性能优化引入未报备的 `unsafe`。
- **拒收条件**：任何 `error:` 输出；任何未经 Lead 书面批准的 `unsafe` 代码块。

### P1：依赖与特性一致性（建议 CI 告警）

#### 1. `cargo build --workspace --all-targets --all-features --locked`
- **目的**：确保 `Cargo.lock` 文件与 `Cargo.toml` 声明的依赖版本一致，防止未经审查的依赖更新引入 CI。
- **抓住什么**：本地 `cargo update` 后忘记提交 `Cargo.lock`。
- **常见失败**：`Cargo.lock` 过期。

#### 2. `cargo tree -d`
- **目的**：检查依赖树中是否存在重复依赖（同一 crate 的多个不兼容版本），这会增加编译体积与潜在冲突。
- **抓住什么**：`A v0.1` 与 `A v0.2` 同时存在于依赖树中。
- **常见失败**：不同依赖项锁定了同一 crate 的不同主版本。

#### 3. `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`
- **目的**：确保项目在“最小特性集”下也能编译通过且无 lint。
- **抓住什么**：代码逻辑错误地依赖了某个“默认开启”的 feature。
- **常见失败**：`#[cfg(feature = "...")]` 使用不当。

### P2：未来兼容性与文档质量（本地自检）

#### 1. `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`
- **目的**：确保文档生成过程中无告警，例如私有字段的公开文档、错误的文档链接等。
- **抓住什么**：文档质量瑕疵。

## (C) 防作弊审计要求（固化）

为防止 Gate 被“糊弄过去”，每次代码审查与 CI 必须包含以下扫描：

1. **扫描 `allow` 扩散**：
   ```bash
   rg -n "\\#\\[allow\\(clippy::" -S .
   ```
   - **拒收条件**：任何未经 Lead 在决策文档中批准的、新增或大范围应用的 `allow`。

2. **扫描 `unsafe` 关键字**：
   ```bash
   rg -n "unsafe fn|unsafe \\{|" -S src/
   ```
   - **拒收条件**：任何未在 `.collab/decisions/DEC__unsafe-usage__....md` 中报备并批准的 `unsafe` 代码。

3. **扫描 CI 脚本**：
   - 检查 `.github/workflows/*.yml` 等文件，确保 `RUSTFLAGS` 未被覆盖、`-D warnings` 未被移除。
