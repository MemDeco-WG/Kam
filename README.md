# Kam

Kam 是一个模块管理工具，提供先进的依赖解析、模块构建和缓存管理功能。

## 特性

### 依赖解析

Kam 提供了受 Python PEP 735 和 uv 包管理器启发的高级依赖解析系统：

- **依赖分组**：将依赖组织到不同的组中（kam、dev 等）
- **组包含**：使用 `include:` 前缀让一个组包含另一个组的所有依赖
- **嵌套包含**：支持多级依赖层次结构
- **循环检测**：自动检测并防止循环依赖
- **详细错误信息**：提供清晰的错误消息，帮助快速定位问题

详细文档请参见 [docs/dependency-resolution.md](docs/dependency-resolution.md)

### 全局缓存系统

受 [uv-cache](https://github.com/astral-sh/uv) 启发的全局缓存机制：

- **平台感知**：自动检测 Android (`/data/adb/kam`) 和非 Android (`~/.kam/`)
- **结构化存储**：分离的 bin、lib、log、profile 目录
- **缓存管理**：查看统计信息、清理缓存
- **高效存储**：Unix 使用符号链接，Windows 使用文件复制

详细文档请参见 [docs/cache-and-venv.md](docs/cache-and-venv.md)

### 虚拟环境

类似 Python virtualenv 的隔离环境：

- **开发环境**：包含开发依赖（`kam sync --dev --venv`）
- **运行时环境**：仅生产依赖
- **跨平台支持**：Unix、Windows、PowerShell 激活脚本
- **路径管理**：自动更新 PATH 和提示符

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

## 命令

### 初始化项目
```bash
kam init my-module --name "My Module" --author "Your Name"
    # 使用仓库模板初始化（将创建一个 module repository 项目结构）
    kam init repo my-repo --name "My Module Repo"
```
仓库模板用于托管多个模块包（packages/），通常配合 `kam publish` 使用以发布模块到本地或远端仓库。

### 同步依赖
```bash
kam sync              # 同步普通依赖
kam sync --dev        # 包含开发依赖
kam sync --dev --venv # 创建虚拟环境
```

### 构建模块
```bash
kam build             # 构建到 dist/ 目录
kam build -o custom/  # 自定义输出目录
```

### 管理缓存
```bash
kam cache info        # 查看缓存信息
kam cache clear       # 清空缓存
kam cache clear-dir log # 清空日志
kam cache path        # 显示缓存路径
```

### 使用虚拟环境
```bash
# 创建并激活
kam sync --venv
source .kam-venv/activate  # Unix
.kam-venv\activate.bat     # Windows CMD
.kam-venv\activate.ps1     # PowerShell

# 停用
deactivate
```

## 快速开始

```bash
# 1. 初始化项目
kam init my-project

# 2. 编辑 kam.toml 添加依赖

# 3. 同步依赖
cd my-project
kam sync --dev --venv

# 4. 激活虚拟环境
source .kam-venv/activate

# 5. 开发工作...

# 6. 构建模块
kam build

# 7. 停用虚拟环境
deactivate
```

## 贡献

1. Fork 仓库
2. 创建功能分支
3. 为新功能添加测试
4. 确保所有测试通过
5. 提交拉取请求

## 许可证

本项目采用 MIT 许可证 - 详情请见 LICENSE 文件。
