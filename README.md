# Kam

Kam 是一个模块管理工具，提供先进的依赖解析和模块构建功能。

## 特性

### 依赖解析

Kam 提供了受 Python PEP 735 和 uv 包管理器启发的高级依赖解析系统：

- **依赖分组**：将依赖组织到不同的组中（normal、dev 等）
- **组包含**：使用 `include:` 前缀让一个组包含另一个组的所有依赖
- **嵌套包含**：支持多级依赖层次结构
- **循环检测**：自动检测并防止循环依赖
- **详细错误信息**：提供清晰的错误消息，帮助快速定位问题

详细文档请参见 [docs/dependency-resolution.md](docs/dependency-resolution.md)

### 示例配置

```toml
[kam.dependency]
# 基础运行时依赖
normal = [
    { id = "core-lib", version = "1.0.0" },
    { id = "utils", version = "2.0.0" }
]

# 开发依赖包含所有运行时依赖
dev = [
    { id = "include:normal" },
    { id = "test-framework", version = "3.0.0" }
]
```

## 贡献

1. Fork 仓库
2. 创建功能分支
3. 为新功能添加测试
4. 确保所有测试通过
5. 提交拉取请求

## 许可证

本项目采用 MIT 许可证 - 详情请见 LICENSE 文件。
