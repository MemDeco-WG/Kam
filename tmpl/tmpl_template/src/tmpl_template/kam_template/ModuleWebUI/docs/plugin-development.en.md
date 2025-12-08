# Plugin Development Guide

## Quick Start

Creating a plugin only requires writing a simple class in `src/plugins/your-plugin/index.js`:

```javascript
class MyPlugin {
  async init(api) {
    // Add button
    api.addButton('header', {
      icon: 'star',
      title: 'My Button',
      action: () => api.showToast('Hello!', 'success')
    });
  }
}

export default MyPlugin;
```

## Common APIs

For function calls, please refer to [Development Guide](develop.en.md)

### Adding Buttons
```javascript
api.addButton('header', {  // Position: header/sidebar/bottom
  icon: 'star',           // Icon name
  title: 'Tooltip text',
  action: () => {}        // Click event
});
```

### Show Messages
```javascript
api.showToast('Message content', 'success');  // success/error/warning/info
```

### Dialogs
```javascript
const ok = await api.showDialog.confirm('Title', 'Content');
const input = await api.showDialog.input('Title', 'Prompt');
```

### Listen to Events
```javascript
api.addHook('page:loaded', (data) => {
  console.log('Page loaded:', data.page);
});
```

### Settings Storage
```javascript
api.setSetting('key', 'value');
const value = api.getSetting('key', 'default value');
```

## Creating Pages

Plugins can create custom pages. For detailed page class development guide, please refer to: [Page Module Development Guide](page-module-development.en.md)

### Page and Plugin Interaction Example

```javascript
class MyPage {
  async onShow() {
    // Trigger custom event for plugins to listen
    if (window.app.pluginManager) {
      await window.app.pluginManager.triggerHook('mypage:shown', {
        page: this,
        data: this.data
      });
    }
  }
}
```

## Complete Example

```javascript
class MyPlugin {
  async init(api) {
    // Add button
    api.addButton('header', {
      icon: 'settings',
      title: 'Plugin Settings',
      action: () => this.showSettings(api)
    });
    
    // Listen to page switching
    api.addHook('page:loaded', (data) => {
      if (data.page === 'home') {
        api.showToast('Welcome to use the plugin!', 'info');
      }
    });
  }
  
  async showSettings(api) {
    const name = await api.showDialog.input('Settings', 'Enter your name:');
    if (name) {
      api.setSetting('userName', name);
      api.showToast('Settings saved', 'success');
    }
  }
}

export default MyPlugin;
```

## Installing Plugins

1. Place plugin files in `src/plugins/plugin-name/index.js`
2. Refresh the page to automatically load

## Debugging

Enter `localStorage.setItem('debugMode', 'true')` in the browser console to enable debug mode.