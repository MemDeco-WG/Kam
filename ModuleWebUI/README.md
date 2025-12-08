# ModuleWebUI / 页面模块管理系统

A modern Android Root universal module management web UI built with Vite — a modular, plugin-friendly frontend for Android Root modules and tooling.

一个基于 Vite 构建的现代化安卓 Root 通用模块管理 Web 界面，提供模块化页面系统、插件开发能力与国际化支持。

---

## Table of Contents / 目录

- [Quick Start / 快速开始](#quick-start--快速开始)
- [Prerequisites / 环境要求](#prerequisites--环境要求)
- [Install & Run / 安装与运行](#install--run--安装与运行)
- [Build & Packaging / 构建与打包](#build--packaging--构建与打包)
- [Dependency Updates / 依赖更新说明](#dependency-updates--依赖更新说明)
- [Development Tips / 开发建议](#development-tips--开发建议)
- [Docs / 文档](#docs--文档)
- [CI / 持续集成](#ci--持续集成)
- [Contributing / 贡献指南](#contributing--贡献指南)
- [License / 许可证](#license--许可证)

---

## Quick Start / 快速开始

Clone the repository, install dependencies, and run the dev server.

克隆仓库、安装依赖并运行开发服务器。

```bash
# Clone
git clone https://github.com/MemDeco-WG/ModuleWebUI.git
cd ModuleWebUI

# Install
npm ci  # preferred in CI
# or
npm install

# Run the dev server
npm run dev
# Open http://localhost:5173 (default Vite port)
```

---

## Prerequisites / 环境要求

- Node.js (LTS recommended) — the project CI targets Node 16, 18, 20, and step used Node 22 for some checks. Use Node >= 16; Node 22 is recommended for local testing.
- npm (>= 8)
- Bash / POSIX shell for `build.sh` (on Windows use WSL / Git Bash)

---

## Install & Run / 安装与运行

- Use `npm ci` in CI environments for reproducible installs, or `npm install` locally.
- Start dev server:

```bash
npm run dev
```

- Preview production build locally:

```bash
npm run build:prod
npm run preview
```

- Use `./build.sh` to build a packaged module and replace `ModuleWebUI` with your module ID:

```bash
# Build for module id "com.example.MyModule"
./build.sh com.example.MyModule
```

Notes:
- `build.sh` will call `npm ci` if `node_modules` does not exist.
- `build.sh` replaces the string `ModuleWebUI` in `src` and `index.html` with the provided module id before running the production build.

---

## Build & Packaging / 构建与打包

- `npm run build` — creates a development build using Vite.
- `npm run build:prod` — creates a production build (minified and optimized).
- `npm run analyze` — helps analyze sizes during build.

The final build artifacts are output to `dist/`. `build.sh` helps prepare a module for packaging by replacing the default internal module ID.

---

## Dependency Updates / 依赖更新说明

This repository maintains dependencies via `package.json`. Recent dependency updates include safe, non-breaking minor/patch bumps. Example of dependencies set in `package.json`:

- `vite`: ^7.0.0 (dev dependency)
- `esbuild`: ^0.25.12
- `fdir`: ^6.5.0
- `glob`: ^11.1.0
- `rollup`: ^4.53.3
- `tinyglobby`: ^0.2.15
- Others (picocolors, picomatch, postcss, ...)

How to check for outdated dependencies:

```bash
npm outdated
```

How to safely update dependencies:

1. Non-breaking updates (safe upgrades):  
   ```bash
   npm update
   ```
   This upgrades to the latest matching semver versions according to `package.json` (no major versions).

2. Major updates (requires manual testing):
   - Use `npm install package@latest` to install a major update.
   - Or use `npx npm-check-updates -u` to update `package.json` to newer major versions, then run `npm install`.
   - After major updates, carefully test builds and runtime behavior.

3. Audit vulnerabilities:
   ```bash
   npm audit
   npm audit fix  # may be used, but pay attention to major changes
   ```

Important: Always run builds & tests (or CI pipeline) after dependency upgrades, and verify changes do not introduce breaking behavior. Some modules have breaking changes across major versions — e.g., `nanoid` major upgrades changed ESM/CommonJS behavior. Be careful when updating major versions.

---

## Development Tips / 开发建议

- Pages & modules: This project organizes page modules independently. Each page module has lifecycle hooks and can be dynamically loaded. See `src/pages.json` and `src/pages/` for examples.
- Plugins: The plugin system allows adding header/footer buttons, UI changes, actions, and listening to events.
- i18n: The project supports multiple locales. Language pack contributions are supported via `src/i18n` and `settings.json`.
- Use the `index.html` as the entry point and the modules in `src/modules/*`.

Troubleshooting:
- If `sed` fails on macOS with `-i` usage, adjust the `build.sh` or run the replacement manually with the macOS `sed` form:
  ```bash
  # For macOS
  find src -name "*.js" -exec sed -i '' "s/ModuleWebUI/${MODID}/g" {} \;
  sed -i '' "s/ModuleWebUI/${MODID}/g" index.html
  ```
- For Windows, use WSL, Git Bash, or modify `build.sh` to use cross-platform tools.

---

## Docs / 文档

Further developer/user documentation is available in the `docs/` folder:

- `docs/README.md` (中文)
- `docs/README.en.md` (English)
- `docs/user.md` | `docs/user-en.md` — user documentation
- `docs/page-module-development.md` | `docs/page-module-development.en.md` — Page module development guide
- `docs/plugin-development.md` | `docs/plugin-development.en.md` — Plugin development guide

Use these to learn how to create new pages, custom plugins, and localization.

---

## CI / 持续集成

This repository includes GitHub Actions CI:

- `.github/workflows/ci.yml` runs build & quality checks.
- CI uses `npm ci` to install dependencies and run `build.sh`.
- The build matrix checks Node 16, 18, and 20; one step uses Node 22 to validate certain tooling (like Vite).

Note: Ensure packages remain compatible with Node versions you plan to support.

---

## Contributing / 贡献

Contributions are welcome.

Suggested workflow:
1. Fork the repository and create a feature branch.
2. Make changes or fixes.
3. Run `npm install` and `npm run dev` to verify local behavior.
4. Build with `./build.sh <MODID>` to confirm packaging.
5. Open a PR with a clear description and link to documentation if adding features.

Please adhere to repository guidelines and ensure your changes are properly tested.

---

## License / 许可证

ModuleWebUI is released under the MIT License. See the `LICENSE` file for full terms.

---

## Acknowledgements / 鸣谢

- Built with Vite, Rollup, and many open-source libraries (see `package.json`).
- Thanks to contributors & community for bug fixes and improvements.

---

If you need further help or a more specific README section (examples, API docs, or CI customization), let me know which area to expand.