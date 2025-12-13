# kam.toml 规范设计文档

## 设计目标

`kam.toml` 的设计旨在：

1. **统一配置**：将模块属性（module.prop）、更新信息（update.json）以及其他元数据统一到一个配置文件中
2. **分层清晰**：采用分层结构，便于理解和维护
3. **向后兼容**：支持现有模块格式的平滑迁移
4. **扩展性强**：允许未来添加新字段而不破坏现有配置
5. **工具友好**：易于解析、验证和生成

## 设计原则

### 1. 分层组织

配置文件采用分层结构，主要分为以下几个顶层节：

- `[prop]`：模块基本属性（必需）
- `[mmrl]`：MMRL 仓库元数据（可选）
- `[kam]`：Kam 平台特定配置（必需）
- `[tmpl]`：模板相关配置（可选）
- `[tool]`：工具扩展配置（可选）

这种分层设计使得：
- 不同用途的配置清晰分离
- 易于理解配置的归属
- 便于工具按需处理不同部分

### 2. 必需 vs 可选

#### 必需字段

必需字段是模块正常运行的最小配置集：

- `[prop]` 节：模块标识和基本信息
- `[prop].id`：模块唯一标识
- `[prop].version`：版本号
- `[prop].versionCode`：版本代码
- `[prop].description`：模块描述
- `[kam]` 节：Kam 平台配置

#### 可选字段

可选字段提供额外的元数据和功能：

- `[mmrl]` 节：完整的仓库元数据（用于分发）
- `[kam.build]`：自定义构建配置
- `[kam.tmpl]`：模板变量定义
- `[kam.workspace]`：工作区配置

**设计考虑**：
- 最小配置应足够简单，便于快速上手
- 完整配置应足够丰富，满足高级需求
- 默认值应合理，减少用户配置负担

### 3. 类型系统

#### 基本类型

- **字符串**：用于文本字段（id、name、description 等）
- **整数**：用于版本代码、API 版本等
- **布尔值**：用于开关字段（metamodule、verified 等）
- **数组**：用于列表字段（keywords、categories、arch 等）
- **表**：用于嵌套配置（build、tmpl、workspace 等）

#### 特殊类型处理

1. **metamodule 字段**：
   - 支持多种输入格式：`true`/`false`、`1`/`0`、`"true"`/`"false"`
   - 提高兼容性，适应不同来源的配置

2. **maintainers 字段**：
   - 支持字符串和对象两种格式
   - 灵活满足简单和复杂场景

3. **架构枚举**：
   - 支持多种别名（如 `arm64`/`aarch64`）
   - 自动规范化到标准值

### 4. 默认值策略

#### 显式默认值

某些字段提供显式默认值，减少配置冗余：

```toml
[kam.build]
target_dir = "dist"  # 默认值
hooks_dir = "hooks"  # 默认值
```

#### 隐式默认值

某些字段在未指定时使用隐式默认值：

- 空数组：`[]`
- 空字符串：`""`
- 零值：`0`
- 假值：`false`

#### 默认值设计原则

1. **合理默认**：默认值应适用于大多数场景
2. **可覆盖**：所有默认值都可以被显式配置覆盖
3. **文档化**：所有默认值都应在规范文档中明确说明

### 5. 验证机制

#### 语法验证

- TOML 格式正确性
- 字段类型匹配
- 必需字段存在性

#### 语义验证

- `id` 字符限制（仅允许字母、数字、下划线、连字符、点）
- `versionCode` 必须为正整数
- 文件路径存在性检查（license_file、readme_file 等）

#### 验证级别

- **错误**：阻止构建的严重问题
- **警告**：建议修复但不阻止构建的问题

### 6. 模板变量支持

在特定字段中支持模板变量替换：

- `{{id}}`：模块 ID
- `{{version}}`：版本号
- `{{versionCode}}`：版本代码

**使用场景**：
- 输出文件名：`output_file = "{{id}}-{{version}}.zip"`
- 路径配置：`source_dir = "src/{{id}}"`

**设计考虑**：
- 模板变量仅在特定字段中生效
- 避免过度使用，保持配置可读性

### 7. 扩展性设计

#### 向后兼容

- 新字段默认为可选
- 保留未使用的字段而不报错
- 支持字段别名（如架构别名）

#### 向前兼容

- 使用版本字段（如 `min_api`、`max_api`）控制兼容性
- 工具应优雅处理未知字段

#### 扩展点

1. **`[tool]` 节**：允许工具存储任意配置
2. **`[kam.tmpl.variables]`**：支持自定义模板变量
3. **`[mmrl.repo.options]`**：支持未来扩展选项

### 8. 命名约定

#### 字段命名

- 使用 `snake_case`（如 `version_code`、`readme_file`）
- 保持与现有格式一致（如 `versionCode` 保持驼峰）
- 使用描述性名称

#### 节命名

- 使用小写字母
- 使用点号分隔层级（如 `mmrl.repo`）
- 保持简洁明了

### 9. 文件组织

#### 必需文件检查

如果配置中指定了文件路径，工具应验证文件存在：

- `license_file`
- `readme_file`
- `changelog_file`
- `icon`（如果为文件路径）

#### 相对路径

所有文件路径均为相对于项目根目录的相对路径。

### 10. 工作区支持

支持类似 Cargo 的工作区概念：

```toml
[kam.workspace]
members = [
    "tmpl/*_template",
    ".",
]
exclude = ["target/"]
```

**设计考虑**：
- 支持 glob 模式匹配
- 支持排除特定路径
- 便于管理多模块项目

## 最佳实践

### 1. 最小配置

对于简单模块，最小配置应包含：

```toml
[prop]
id = "MyModule"
name = "My Module"
version = "1.0.0"
versionCode = 1
author = "Author Name"
description = "Module description"

[kam]
module_type = "kam"
```

### 2. 完整配置

对于要发布的模块，建议包含完整的 `[mmrl.repo]` 配置：

```toml
[prop]
# ... 基本属性

[mmrl.repo]
license = "MIT"
license_file = "LICENSE"
homepage = "https://github.com/user/repo"
repository = "https://github.com/user/repo"
readme_file = "README.md"
changelog_file = "CHANGELOG.md"
categories = ["tools"]
keywords = ["module", "android"]
arch = ["arm64-v8a"]
min_api = 21
max_api = 35
```

### 3. 构建配置

对于需要自定义构建的模块：

```toml
[kam.build]
source_dir = "src/custom"
target_dir = "dist"
output_file = "{{id}}-{{version}}.zip"
hooks_dir = "hooks"

[[kam.build.extra_includes]]
source = "docs/README.md"
dest = "README.md"
```

### 4. 模板配置

对于模板模块：

```toml
[kam]
module_type = "template"

[kam.tmpl]
used_template = "kam_template"

[kam.tmpl.variables.module_name]
var_type = "string"
required = true
help = "The display name of the module"
example = "My Awesome Module"

[kam.tmpl.variables.author]
var_type = "string"
required = true
default = "Unknown"
```

### 5. 版本管理

- 使用语义化版本（Semantic Versioning）
- `versionCode` 建议使用时间戳，确保单调递增
- 在 `updateJson` 中提供更新信息

### 6. 元数据完整性

- 填写所有相关的元数据字段
- 提供有效的 URL 链接
- 使用合适的分类和关键字
- 添加屏幕截图（如适用）

## 迁移指南

### 从 module.prop 迁移

```properties
# module.prop
id=MyModule
name=My Module
version=1.0.0
versionCode=1
author=Author
description=Description
```

转换为：

```toml
[prop]
id = "MyModule"
name = "My Module"
version = "1.0.0"
versionCode = 1
author = "Author"
description = "Description"
```

### 从 update.json 迁移

```json
{
  "version": "1.0.0",
  "versionCode": 1,
  "zipUrl": "https://...",
  "changelog": "https://..."
}
```

相关信息迁移到 `[mmrl.repo]` 节。

## 未来扩展方向

### 1. 多环境配置

支持开发、测试、生产环境的配置：

```toml
[kam.build.profiles.dev]
target_dir = "dist/dev"

[kam.build.profiles.prod]
target_dir = "dist/prod"
```

### 2. 依赖管理

支持模块依赖声明：

```toml
[kam.dependencies]
required = ["ModuleA", "ModuleB"]
optional = ["ModuleC"]
```

### 3. 构建脚本配置

支持自定义构建脚本：

```toml
[kam.build.scripts]
pre_build = "scripts/pre-build.sh"
post_build = "scripts/post-build.sh"
```

### 4. 测试配置

支持测试相关配置：

```toml
[kam.test]
enabled = true
test_dir = "tests"
```

## 工具支持

### 验证工具

- `kam validate`：验证配置文件格式和语义
- 集成到 CI/CD 流程中

### 生成工具

- `kam init`：交互式生成初始配置
- `kam toml get/set`：读取和修改配置值

### 格式化工具

- 自动格式化 TOML 文件
- 保持一致的缩进和排序

## 总结

`kam.toml` 的设计遵循以下核心理念：

1. **简单易用**：最小配置即可开始
2. **功能完整**：支持复杂场景和高级需求
3. **清晰分层**：配置结构清晰，易于理解
4. **扩展灵活**：支持未来扩展而不破坏兼容性
5. **工具友好**：易于解析、验证和生成

通过遵循这些设计原则和最佳实践，`kam.toml` 能够满足从简单模块到复杂项目的各种需求。
