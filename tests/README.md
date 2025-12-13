# 测试文档

本目录包含 Kam 项目的所有测试文件。

## 目录结构

```
tests/
├── README.md              # 本文件
├── common/                # 测试公共模块
│   └── mod.rs            # 测试辅助函数
├── utils_tests.rs         # 工具函数测试
├── kam_toml_tests.rs      # KamToml 配置测试
├── version_tests.rs       # 版本管理测试
├── toml_tests.rs          # TOML 操作测试
├── export_tests.rs        # 导出功能测试
└── integration_tests.rs   # 集成测试
```

## 运行测试

### 运行所有测试

```bash
cargo test
```

### 运行特定测试文件

```bash
cargo test --test utils_tests
cargo test --test kam_toml_tests
cargo test --test version_tests
cargo test --test toml_tests
cargo test --test export_tests
cargo test --test integration_tests
```

### 运行特定测试用例

```bash
cargo test test_pattern_matches_directory_prefix
cargo test test_validate_version_valid
```

### 显示测试输出

```bash
cargo test -- --nocapture
```

### 运行测试并显示覆盖率（需要 cargo-tarpaulin）

```bash
cargo install cargo-tarpaulin
cargo tarpaulin --out Html
```

## 测试类型

### 单元测试

- **utils_tests.rs**: 测试工具函数，包括模式匹配、环境变量处理等
- **kam_toml_tests.rs**: 测试 KamToml 配置的加载、保存和操作
- **version_tests.rs**: 测试版本号验证和 bump 功能
- **toml_tests.rs**: 测试 TOML 文件的读取、写入和操作

### 功能测试

- **export_tests.rs**: 测试各种导出格式（module.prop, update.json, module.json, repo.json）

### 集成测试

- **integration_tests.rs**: 测试多个模块协同工作的完整工作流

## 编写新测试

1. 在 `tests/` 目录下创建新的测试文件
2. 使用 `#[test]` 属性标记测试函数
3. 使用 `assert!`, `assert_eq!`, `assert_ne!` 等宏进行断言
4. 使用 `tempfile::TempDir` 创建临时目录进行文件操作测试

### 示例

```rust
#[test]
fn test_my_function() {
    let result = my_function();
    assert_eq!(result, expected_value);
}
```

## 测试最佳实践

1. **独立性**: 每个测试应该独立运行，不依赖其他测试的状态
2. **可重复性**: 测试应该在任何环境下都能通过
3. **清晰性**: 测试名称应该清楚地描述测试的内容
4. **快速性**: 测试应该快速执行
5. **隔离性**: 使用临时目录和文件，避免影响实际文件系统

## 依赖

测试使用以下依赖（在 `Cargo.toml` 的 `[dev-dependencies]` 中）：

- `tempfile`: 创建临时文件和目录
- `serial_test`: 串行化测试执行（如果需要）

## 持续集成

测试会在 CI/CD 流程中自动运行。确保所有测试通过后再提交代码。
