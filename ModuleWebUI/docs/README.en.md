# ModuleWebUI Page Module Management System

A modern optional web interface for viewing and interacting with module pages built with Vite. Note: ModuleWebUI is UI-only and does NOT provide runtime module management (installation/uninstallation/enabling/disabling of modules). Kam focuses on module initialization and packaging only.

[中文版本](./README.md)

## Quick Start

### Installation and Running

```bash
# Clone the project
git clone https://github.com/APMMDEVS/ModuleWebUI.git
cd ModuleWebUI

./build.sh
```
GitHub Actions automatic build uses the repository name as the module name by default, which you can change.

## More Documentation

- [User Documentation](./user-en.md)
- [Page Module Development Guide](./page-module-development.en.md)
- [Plugin Development Guide](./plugin-development.en.md)

## Detailed Feature Introduction

### Core Features
- Supports KernelSU web UI (UI-only). ModuleWebUI is optional and does not provide runtime module management; it is intended for visualizing and interacting with module pages rather than managing module lifecycle.
- Supports executing shell commands.

### Page Module System
- Each page is an independent module
- Complete module lifecycle hooks
- Supports custom page creation
- Pages can add interactions, listen to events, modify pages, etc.

### Plugin System
- Supports custom plugin development
- Plugins can add buttons, listen to events, modify pages, etc.

### Internationalization System
- Supports multi-language switching
- Plugins can add new language packs

## License

This project is licensed under the MIT License - see the [LICENSE](../LICENSE) file for details.