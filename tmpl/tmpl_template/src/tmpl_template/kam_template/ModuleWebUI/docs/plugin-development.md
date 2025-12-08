# 插件开发指南

## 快速开始

创建插件只需要在 `src/plugins/your-plugin/index.js` 中编写一个简单的类：

```javascript
class MyPlugin {
  async init(api) {
    // 添加按钮
    api.addButton('header', {
      icon: 'star',
      title: '我的按钮',
      action: () => api.showToast('Hello!', 'success')
    });
  }
}

export default MyPlugin;
```

## 常用API

函数调用等请参考[开发指南](develop.md)

### 添加按钮
```javascript
api.addButton('header', {  // 位置: header/sidebar/bottom
  icon: 'star',           // 图标名
  title: '提示文本',
  action: () => {}        // 点击事件
});
```

### 显示消息
```javascript
api.showToast('消息内容', 'success');  // success/error/warning/info
```

### 对话框
```javascript
const ok = await api.showDialog.confirm('标题', '内容');
const input = await api.showDialog.input('标题', '提示');
```

### 监听事件
```javascript
api.addHook('page:loaded', (data) => {
  console.log('页面已加载:', data.page);
});
```

### 设置存储
```javascript
api.setSetting('key', 'value');
const value = api.getSetting('key', '默认值');
```

## 创建页面

插件可以创建自定义页面。页面类的详细开发指南请参考：[页面模块开发指南](page-module-development.md)

### 页面与插件交互示例

```javascript
class MyPage {
  async onShow() {
    // 触发自定义事件供插件监听
    if (window.app.pluginManager) {
      await window.app.pluginManager.triggerHook('mypage:shown', {
        page: this,
        data: this.data
      });
    }
  }
}
```

## 完整示例

```javascript
class MyPlugin {
  async init(api) {
    // 添加按钮
    api.addButton('header', {
      icon: 'settings',
      title: '插件设置',
      action: () => this.showSettings(api)
    });
    
    // 监听页面切换
    api.addHook('page:loaded', (data) => {
      if (data.page === 'home') {
        api.showToast('欢迎使用插件！', 'info');
      }
    });
  }
  
  async showSettings(api) {
    const name = await api.showDialog.input('设置', '输入您的名字:');
    if (name) {
      api.setSetting('userName', name);
      api.showToast('设置已保存', 'success');
    }
  }
}

export default MyPlugin;
```

## 安装插件

1. 将插件文件放在 `src/plugins/插件名/index.js`
2. 刷新页面即可自动加载

## 调试

在浏览器控制台输入 `localStorage.setItem('debugMode', 'true')` 启用调试模式。