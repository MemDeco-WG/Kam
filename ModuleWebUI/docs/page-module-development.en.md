# Page Module Development Guide

## Quick Start

Creating a page only requires implementing a simple class:

```javascript
class MyPage {
  constructor() {
    // Constructor: Initialize page data and state
    this.data = {};
    this.eventListeners = [];
  }

  async render() {
    return `
      <div class="my-page">
        <h2>My Page</h2>
        <p>Page content</p>
      </div>
    `;
  }
}

export { MyPage };
```

## Page Class

### CSS Styles
app.js will automatically load the corresponding page CSS file from the `src/assets/css/pages/` directory, with the file name matching the page class name, for example `MyPage.css`

### Required Methods

- `render()`: Returns page HTML content

### Optional Methods

- `onShow()`: Called when page is displayed
- `cleanup()`: Called when page is cleaned up
- `getPageActions()`: Returns page action button configuration

# Interactions (The following content is recommended to add internationalization support)

For function calls, please refer to [Development Guide](develop.en.md)

## Constructor

The constructor is called when the page class is instantiated, used to initialize page state:

```javascript
class MyPage {
  constructor() {
    // Initialize page data
    this.data = {
      items: [],
      loading: false
    };
    
    // Initialize event listener array (for cleanup)
    this.eventListeners = [];
    
    // Initialize other states
    this.isInitialized = false;
  }
}
```

## Adding Interactions

```javascript
class MyPage {
  async render() {
    return `
      <div class="my-page">
        <h2>My Page</h2>
        <button id="my-btn">Click Me</button>
      </div>
    `;
  }
  
  async onShow() {
    // Add events when page is displayed, you can also add data reading functionality
    document.getElementById('my-btn').onclick = () => {
      window.core.showToast('Button clicked!', 'success');
    };
  }
}
```

## Adding Buttons (Top Bar Buttons)

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

## Register Page

Add in `src/pages.json`:

```json
{
  "my-page": {
    "title": "MyPage",
    "icon": "dashboard",
    "file": "my-page.js"
  }
}
```

## Adding Styles

Create a style file at `src/assets/css/pages/my-page.css`:

```css
.my-page {
  padding: 24px;
}

.my-page h2 {
  color: var(--md-sys-color-primary);
}
```

## Internationalization Support

To support internationalization, you need to add corresponding language files in the `src/i18n/modules/` directory. For example, to add Chinese support:

```json
{
  "myPage": {
    "title": "My Page",
    "content": "This is my page content"
  },
  "nav": {
    "MyPage": "My Interface"
  }
}
```