# User Documentation

## Quick Start

### Home Page

#### Module Status Display
The module status display on the home page is implemented by reading the `info.txt` file in the module root directory.
The format of the `info.txt` file is:
```
status=unknown/running/stopped/error
pid=
startTime=
lastUpdate=
```

The status card displays:
- Module running status (Running/Stopped/Error/Normal Exit/Unknown)
- Process ID (if available)
- Start time (if available)
- Last update time

#### Custom Action Buttons
The home page provides three custom action buttons below the status card, which developers can bind to custom functions as needed:
**Note:** Custom action buttons are hidden by default. You need to set `this.AllowCustomActions = true;` in `src/pages/home.js` to enable them.

1. **Extension Button** (extension icon)
   - Default display: "Extension"
   - Can be used to bind module-specific extension functions

2. **Build Tools Button** (build icon)
   - Default display: "Build Tools"
   - Can be used to bind build, compilation and related functions

3. **Debug Tools Button** (tune icon)
   - Default display: "Debug Tools"
   - Can be used to bind debugging, diagnostic and related functions

**Custom Button Event Binding:**
Developers can modify the button click event handling logic in the `bindCustomActions()` method of `src/pages/home.js`. By default, clicking a button will show a Toast notification and output logs in debug mode.

```javascript
// Example: Custom button 1 event handling
if (action1) {
  action1.addEventListener('click', () => {
    // Add your custom logic here
    window.core.showToast("Execute custom function", "success");
    // Execute specific function code...
  });
}
```

### Logs

#### Log File Management
The logs page supports viewing and managing module log files:

**Log File Location:**
- KSU Environment: `${MODULE_PATH}/logs/` directory
- Browser Environment: Uses mock data

**Supported Log Files:**
- `module.log` - Main module log
- `system.log` - System log
- `error.log` - Error log
- `debug.log` - Debug log
- Other custom log files

#### Log Level Filtering

The logs page provides log level filtering functionality:

**Supported Log Levels:**
- **All Levels** - Show all logs
- **Debug** (DEBUG) - Debug information
- **Info** (INFO) - General information
- **Warning** (WARN) - Warning information
- **Error** (ERROR) - Error information

**Filtering Features:**
- Real-time filtering without reloading
- Maintains virtual scrolling performance
- Shows filtering result statistics
- Supports both mock data and actual log files

#### Log Display Features

**Virtual Scrolling:**
- Supports high-performance display of large files
- Dynamically loads content in visible area
- Smooth scrolling experience

**Log Format Parsing:**
- Automatically recognizes log levels
- Formatted timestamp display
- Log level badge display

**Page Operations:**
1. **Select File** - Switch between different log files
2. **Refresh** - Reload current log file
3. **Clear** - Clear currently displayed log content

#### Mock Data Support

In browser environment, the logs page provides mock data functionality:

```javascript
// Mock log data structure
{
  level: "INFO",    // Log level
  message: "...",   // Log message
  timestamp: "..." // Timestamp
}
```

**Mock File Types:**
- `module.log` - Contains various levels of module logs
- `system.log` - System-related logs
- `error.log` - Mainly contains errors and warnings
- `debug.log` - Mainly contains debug information

#### Internationalization Support

The logs page fully supports internationalization:
- Automatic interface text translation
- Localized log level labels
- Localized error messages

### Settings

#### Configuration Files
The settings page requires two configuration files:

1. **settings.json** - WebUI configuration file, located in module root directory
2. **config.sh** - Shell configuration file, located in module root directory (auto-generated)

#### settings.json Format
```json
{
  "settings": {
    "DEBUG_MODE": {
      "type": "boolean",
      "default": false,
      "title": "Debug Mode",
      "description": "Enable debug mode to output more debug information in console.",
      "i18n": {
        "title": "settings.debugMode",
        "description": "settings.debugModeDesc"
      }
    },
    "LOG_LEVEL": {
      "type": "select",
      "default": "info",
      "title": "Log Level",
      "description": "Set log output level",
      "choices": [
        {
          "value": "debug",
          "label": "Debug",
          "i18n": "settings.logLevel.debug"
        },
        {
          "value": "info",
          "label": "Info",
          "i18n": "settings.logLevel.info"
        },
        {
          "value": "warn",
          "label": "Warning",
          "i18n": "settings.logLevel.warn"
        },
        {
          "value": "error",
          "label": "Error",
          "i18n": "settings.logLevel.error"
        }
      ]
    },
    "MODULE_NAME": {
      "type": "input",
      "default": "MyModule",
      "title": "Module Name",
      "description": "Set module display name"
    }
  }
}
```

#### Supported Setting Types

1. **boolean** - Boolean switch
   - Rendered as checkbox
   - Value is `true` or `false`

2. **select** - Dropdown selection
   - Requires `choices` array
   - Each option contains `value`, `label` and optional `i18n`

3. **input** - Text input
   - Rendered as text input box
   - Supports placeholder showing default value

#### Internationalization Support

Settings support multiple internationalization methods:

1. **JSON Embedded Translation** (Recommended):
```json
{
  "title": "Default Title",
  "translations": {
    "zh": {
      "title": "Chinese Title",
      "description": "Chinese Description"
    },
    "en": {
      "title": "English Title",
      "description": "English Description"
    }
  }
}
```

2. **i18n Key Reference**:
```json
{
  "i18n": {
    "title": "settings.myTitle",
    "description": "settings.myDescription"
  }
}
```

#### Configuration File Processing

**Browser Environment:**
- Settings saved to `localStorage`
- Key name: `modulewebui_settings`

**KSU Environment:**
- Settings saved to `config.sh` file
- Auto-generates Shell variable format
- Supports comments and quote escaping

#### Page Operations

The settings page provides three operation buttons:

1. **Refresh** - Reload settings configuration
2. **Reset** - Restore all settings to default values
3. **Save** - Save current settings

#### Auto Save

- Settings are automatically marked as changed after modification
- Manual save button click required for persistence
- Supports resetting unsaved changes

#### Shell Configuration File Format

Generated `config.sh` file format:
```bash
#!/bin/bash

# ModuleWebUI Configuration
# Generated automatically - do not edit manually

# Debug Mode
# Enable debug mode to output more debug information in console
DEBUG_MODE="false"

# Log Level
# Set log output level
LOG_LEVEL="info"

# Module Name
# Set module display name
MODULE_NAME="MyModule"
```

#### Error Handling

- Shows error state when configuration file loading fails
- Shows error notification when saving fails
- Supports retry mechanism

## Development Guide

### Internationalization Configuration

#### Language File Locations
- `src/i18n/zh.json` - Chinese translation
- `src/i18n/en.json` - English translation
- `src/i18n/ru.json` - Russian translation

#### Adding New Translation Text

1. Add key-value pairs in corresponding language file:
```json
{
  "myModule": {
    "title": "My Module",
    "description": "This is a sample module"
  }
}
```

2. Use in code:
```javascript
const title = window.i18n.t("myModule.title");
```

### Debug Features

#### Enable Debug Mode

In browser environment, you can enable debugging through:

```javascript
// Execute in browser console
window.core.setDebugMode(true);

// Or use in code
if (window.core.isDebugMode()) {
  window.core.logDebug("Debug info", "MODULE_NAME");
}
```

#### Debug Log Output

```javascript
// Different levels of log output
window.core.logDebug("Debug info", "TAG");
window.core.logInfo("General info", "TAG");
window.core.logWarn("Warning info", "TAG");
window.core.logError("Error info", "TAG");
```

### Environment Detection

#### Detect Runtime Environment

```javascript
// Check if running in KSU environment
if (window.core.isKSUEnvironment()) {
  // KSU environment specific logic
  window.core.execCommand("ls -la", (output, success) => {
    console.log(output);
  });
} else {
  // Browser environment logic
  console.log("Running in browser environment");
}
```

### Best Practices

#### Page Development

1. **Inherit Basic Structure:**
   - Implement `render()` method to return HTML
   - Implement `onShow()` method to handle page display logic
   - Implement `getPageActions()` to return page action buttons
   - Implement `cleanup()` method to clean up event listeners

2. **Event Management:**
   - Use `Map` to store event handler references
   - Remove all event listeners in `cleanup()`
   - Avoid memory leaks

3. **Internationalization Support:**
   - All user-visible text should support internationalization
   - Use `window.i18n.t()` to get translated text
   - Organize text structure well in language files

4. **Error Handling:**
   - Use `window.core.showError()` to display errors
   - Use `window.core.showToast()` to display notifications
   - Provide user-friendly error messages

#### Performance Optimization

1. **Virtual Scrolling:**
   - Use virtual scrolling for large amounts of data
   - Only render content in visible area
   - Dynamically calculate scroll position

2. **Event Throttling:**
   - Use throttling for frequently triggered events
   - Avoid excessive rendering

3. **Memory Management:**
   - Clean up unnecessary DOM references promptly
   - Remove event listeners
   - Avoid memory leaks caused by closures

### Troubleshooting

#### Common Issues

1. **Settings Not Taking Effect:**
   - Check if `settings.json` format is correct
   - Confirm configuration file path is correct
   - Check browser console error messages

2. **Internationalization Text Not Displaying:**
   - Check if language file has corresponding key-value
   - Confirm `window.i18n.t()` call is correct
   - Check language file JSON format

3. **Logs Not Displaying:**
   - Confirm log file path is correct
   - Check file permissions
   - Check if network requests are successful

4. **Custom Buttons Not Displaying:**
   - Confirm `this.AllowCustomActions = true` is set
   - Check button binding logic
   - Check if CSS styles are loaded correctly