
## 核心功能 (window.core)

```javascript
// Toast 消息
window.core.showToast('消息内容', 'success'); // success/error/warning/info

// 通用Shell命令执行（例，output是执行命令返回的数据）
window.core.execCommand('ls -la', (output, isSuccess, details) => {
  if (isSuccess) {
    console.log('命令输出:', output);
  } else {
    console.error('命令失败:', output);
  }
});

// Promise方式执行命令
const result = await window.core.exec('pwd');
console.log('当前目录:', result.stdout);

// 错误显示
window.core.showError('错误信息', '上下文');

// 调试日志
window.core.logDebug('调试信息', 'PAGE');

// 检查调试模式
if (window.core.isDebugMode()) {
  console.log('调试模式已启用');
}

// 检查KSU环境（支持MMRL,APATCH,SUKISU）
if (window.core.isKSUEnvironment()) {
  console.log('运行在KernelSU WebUI环境中');
}
```


## 国际化 (window.i18n)

```javascript
// 获取翻译文本
const text = window.i18n.t('myPage.title');

// 格式化文本
const formatted = window.i18n.t('myPage.welcome', { name: 'User' });
```

你需要在插件的`i18n/modules`目录下添加翻译文件，例如`en.json`：
```json
{
  "myPage": {
    "title": "My Page",
    "welcome": "Welcome, {name}!"
  }
}
```

## 对话框 (window.DialogManager)

```javascript
// 确认对话框
const confirmed = await window.DialogManager.showConfirm('标题', '内容');

// 输入对话框
const input = await window.DialogManager.showInput('标题', '提示文本', '占位符', '默认值');

// 列表选择对话框
const selected = await window.DialogManager.showList('选择项目', [
  { text: '选项1', value: 'option1' },
  { text: '选项2', value: 'option2' }
]);

// 通用对话框
await window.DialogManager.showGeneric({
  title: '自定义标题',
  content: '<div>自定义内容</div>',
  buttons: [
    { text: '确定', action: () => console.log('确定') },
    { text: '取消', action: () => console.log('取消') }
  ],
  closable: true
});

// 无标题对话框
await window.DialogManager.showPopup({
  content: '<div>弹出内容</div>',
  closable: true
});

// 简化API方式
const result = await window.DialogManager.dialog.confirm('标题', '内容');
const userInput = await window.DialogManager.dialog.input('标题', '消息');
```

## Debug 支持

### 启用调试模式

在webui设置中启用调试模式或在浏览器控制台输入：
```javascript
localStorage.setItem('debugMode', 'true');
```

### 使用调试功能

```javascript
class MyPage {
  async onShow() {
    // 调试日志
      window.core.logDebug('页面显示', 'PAGE');
  }
}
```
