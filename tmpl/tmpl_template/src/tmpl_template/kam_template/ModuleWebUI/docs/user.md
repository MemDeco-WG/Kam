# 用户文档

## 快速开始

### 首页

#### 模块状态显示
首页的模块状态查看通过读取模块根目录下的`info.txt`文件来实现。
`info.txt`文件的格式为：
```
status=unknown/running/stopped/error
pid=
startTime=
lastUpdate=
```

状态卡片显示内容包括：
- 模块运行状态（运行中/已停止/运行异常/正常退出/状态未知）
- 进程ID（如果有）
- 启动时间（如果有）
- 最后更新时间

#### 自定义操作按钮
首页状态卡片下方提供三个自定义操作按钮，开发者可以根据需要绑定自定义功能：
**注意：** 自定义操作按钮默认是不显示的，需要在`src/pages/home.js`中设置`this.AllowCustomActions = true;`来启用。

1. **扩展功能按钮**（extension图标）
   - 默认显示"扩展功能"
   - 可用于绑定模块特有的扩展功能

2. **构建工具按钮**（build图标）
   - 默认显示"构建工具"
   - 可用于绑定构建、编译等相关功能

3. **调试工具按钮**（tune图标）
   - 默认显示"调试工具"
   - 可用于绑定调试、诊断等相关功能

**自定义按钮事件绑定：**
开发者可以在`src/pages/home.js`的`bindCustomActions()`方法中修改按钮的点击事件处理逻辑。默认情况下，点击按钮会显示Toast提示并在调试模式下输出日志。

```javascript
// 示例：自定义按钮1的事件处理
if (action1) {
  action1.addEventListener('click', () => {
    // 在这里添加你的自定义逻辑
    window.core.showToast("执行自定义功能", "success");
    // 执行具体的功能代码...
  });
}
```

### 日志

**日志文件位置**：`${MODULE_PATH}/logs/` 目录

### 设置

#### 配置文件
设置页面需要两个配置文件：

1. **settings.json** - WebUI配置文件，位于模块根目录
2. **config.sh** - Shell配置文件，位于模块根目录

#### settings.json 格式（例子）
```json
{
  "settings": {
    "DEBUG_MODE": {
      "type": "boolean",
      "default": false,
      "title": "调试模式",
      "description": "启用调试模式，会在控制台输出更多调试信息。",
      "i18n": {
        "title": "settings.debugMode",
        "description": "settings.debugModeDesc"
      }
    },
    "LOG_LEVEL": {
      "type": "select",
      "default": "info",
      "title": "日志级别",
      "description": "设置日志输出级别",
      "choices": [
        {
          "value": "debug",
          "label": "调试",
          "i18n": "settings.logLevel.debug"
        },
        {
          "value": "info",
          "label": "信息",
          "i18n": "settings.logLevel.info"
        },
        {
          "value": "warn",
          "label": "警告",
          "i18n": "settings.logLevel.warn"
        },
        {
          "value": "error",
          "label": "错误",
          "i18n": "settings.logLevel.error"
        }
      ]
    },
    "MODULE_NAME": {
      "type": "input",
      "default": "MyModule",
      "title": "模块名称",
      "description": "设置模块显示名称"
    }
  }
}
```

#### 支持的设置类型

1. **boolean** - 布尔开关
   - 渲染为复选框
   - 值为 `true` 或 `false`

2. **select** - 下拉选择
   - 需要提供 `choices` 数组
   - 每个选项包含 `value`、`label` 和可选的 `i18n`

3. **input** - 文本输入
   - 渲染为文本输入框
   - 支持占位符显示默认值

#### 国际化支持

设置项支持多种国际化方式：

1. **JSON内嵌翻译**（推荐）：
```json
{
  "title": "默认标题",
  "translations": {
    "zh": {
      "title": "中文标题",
      "description": "中文描述"
    },
    "en": {
      "title": "English Title",
      "description": "English Description"
    }
  }
}
```

2. **i18n键引用**：
```json
{
  "i18n": {
    "title": "settings.myTitle",
    "description": "settings.myDescription"
  }
}
```