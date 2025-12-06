# Development Documentation

## Core Functions (window.core)

```javascript
// Toast messages
window.core.showToast('Message content', 'success'); // success/error/warning/info

// General Shell command execution (example, output is the data returned by executing the command)
window.core.execCommand('ls -la', (output, isSuccess, details) => {
  if (isSuccess) {
    console.log('Command output:', output);
  } else {
    console.error('Command failed:', output);
  }
});

// Promise-based command execution
const result = await window.core.exec('pwd');
console.log('Current directory:', result.stdout);

// Error display
window.core.showError('Error message', 'Context');

// Debug logging
window.core.logDebug('Debug info', 'PAGE');

// Check debug mode
if (window.core.isDebugMode()) {
  console.log('Debug mode is enabled');
}

// Check KSU environment (supports MMRL, APATCH, SUKISU)
if (window.core.isKSUEnvironment()) {
  console.log('Running in KernelSU WebUI environment');
}
```

## Internationalization (window.i18n)

```javascript
// Get translated text
const text = window.i18n.t('myPage.title');

// Format text
const formatted = window.i18n.t('myPage.welcome', { name: 'User' });
```

You need to add translation files in the plugin's `i18n/modules` directory, for example `en.json`:
```json
{
  "myPage": {
    "title": "My Page",
    "welcome": "Welcome, {name}!"
  }
}
```

## Dialogs (window.DialogManager)

```javascript
// Confirmation dialog
const confirmed = await window.DialogManager.showConfirm('Title', 'Content');

// Input dialog
const input = await window.DialogManager.showInput('Title', 'Prompt text', 'Placeholder', 'Default value');

// List selection dialog
const selected = await window.DialogManager.showList('Select item', [
  { text: 'Option 1', value: 'option1' },
  { text: 'Option 2', value: 'option2' }
]);

// Generic dialog
await window.DialogManager.showGeneric({
  title: 'Custom Title',
  content: '<div>Custom content</div>',
  buttons: [
    { text: 'OK', action: () => console.log('OK') },
    { text: 'Cancel', action: () => console.log('Cancel') }
  ],
  closable: true
});

// Titleless dialog
await window.DialogManager.showPopup({
  content: '<div>Popup content</div>',
  closable: true
});

// Simplified API approach
const result = await window.DialogManager.dialog.confirm('Title', 'Content');
const userInput = await window.DialogManager.dialog.input('Title', 'Message');
```

## Debug Support

### Enable Debug Mode

Enable debug mode in webui settings or enter in browser console:
```javascript
localStorage.setItem('debugMode', 'true');
```

### Using Debug Features

```javascript
class MyPage {
  async onShow() {
    // Debug logging
    window.core.logDebug('Page displayed', 'PAGE');
  }
}
```