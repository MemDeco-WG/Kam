# 页面模块开发指南

## 快速开始

创建页面只需要实现一个简单的类：

```javascript
class MyPage {
  constructor() {
    // 构造函数：初始化页面数据和状态
    this.data = {};
    this.eventListeners = [];
  }

  async render() {
    return `
      <div class="my-page">
        <h2>我的页面</h2>
        <p>页面内容</p>
      </div>
    `;
  }
}

export { MyPage };
```
## 页面类

### CSS 样式
app.js会自动加载`src/assets/css/pages/`目录下的对应页面css文件，文件名与页面类名相同，例如`MyPage.css`

### 必需方法

- `render()`: 返回页面HTML内容

### 可选方法

- `onShow()`: 页面显示时调用
- `cleanup()`: 页面清理时调用
- `getPageActions()`: 返回页面操作按钮配置


# 交互（以下内容建议添加国际化支持）

函数调用等请参考[开发指南](develop.md)

## Constructor（构造函数）

构造函数在页面类实例化时调用，用于初始化页面状态：

```javascript
class MyPage {
  constructor() {
    // 初始化页面数据
    this.data = {
      items: [],
      loading: false
    };
    
    // 初始化事件监听器数组（用于清理）
    this.eventListeners = [];
    
    // 初始化其他状态
    this.isInitialized = false;
  }
}
```

## 添加交互

```javascript
class MyPage {
  async render() {
    return `
      <div class="my-page">
        <h2>我的页面</h2>
        <button id="my-btn">点击我</button>
      </div>
    `;
  }
  
  async onShow() {
    // 页面显示时添加事件 你也可以添加读取数据等功能
    document.getElementById('my-btn').onclick = () => {
      window.core.showToast('按钮被点击了！', 'success');
    };
  }
}
```

## 添加按钮（顶栏按钮）

```javascript
class MyPage {
  getPageActions() {
    return [
      {
        icon: 'refresh',
        title: 'Refresh',
        action: () => window.core.showToast('Refreshed', 'success')
      }
    ];
  }
}
```

## 注册页面

在 `src/pages.json` 中添加：

```json
{
  "my-page": {
    "title": "MyPage",
    "icon": "dashboard",
    "file": "my-page.js"
  }
}
```

## 添加样式

在 `src/assets/css/pages/my-page.css` 创建样式文件：

```css
.my-page {
  padding: 24px;
}

.my-page h2 {
  color: var(--md-sys-color-primary);
}
```

## 国际化支持

为了支持国际化，你需要在 `src/i18n/modules/` 目录下添加对应的语言文件。例如，添加中文支持：

```json
{
  "myPage": {
    "title": "我的页面",
    "content": "这是我的页面内容"
  },
  "nav": {
    "MyPage": "我的界面"
  }
}
```
