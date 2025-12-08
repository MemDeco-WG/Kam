# Kam Build Hooks

Hooks allow you to run custom scripts at different stages of the build process. Kam provides a flexible hook system with shared utilities and environment variables.

## Built-in Hooks

**Properties synced**:
- `id`
- `name`
- `version`
- `versionCode`
- `author`
- `description`
- `updateJson` (if set)

This hook runs automatically before every build and is included in the standard module templates (`kam_template`, `meta_template`).

## Execution behavior

Kam executes hook files by directly invoking the file and defers to the operating system (or the file itself, via shebang or file associations) to determine how it runs. The hook runner intentionally does not attempt to pick or call interpreters based on the platform or file extension. Ensure your hook scripts are runnable on your target environment (for example: add `#!/bin/sh` and `chmod +x` for Unix-like systems, or run shell scripts via WSL/Git Bash on Windows).

## Environment Variables

When hooks are executed, Kam injects the following environment variables, which you can use in your scripts:

| Variable | Description |
|----------|-------------|
| `KAM_PROJECT_ROOT` | Absolute path to the project root directory. |
| `KAM_HOOKS_ROOT` | Absolute path to the hooks directory. Useful for sourcing shared scripts. |
| `KAM_MODULE_ROOT` | Absolute path to the module source directory (e.g. `src/<id>`). |
| `KAM_WEB_ROOT` | Absolute path to the module webroot directory (`<module_root>/webroot`). |
| `KAM_DIST_DIR` | Absolute path to the build output directory (e.g. `dist`). Useful for uploading artifacts. |
| `KAM_MODULE_ID` | The module ID defined in `kam.toml`. |
| `KAM_MODULE_VERSION` | The module version. |
| `KAM_MODULE_VERSION_CODE` | The module version code. |
| `KAM_MODULE_NAME` | The module name. |
| `KAM_MODULE_AUTHOR` | The module author. |
| `KAM_MODULE_DESCRIPTION` | The module description. |
| `KAM_MODULE_UPDATE_JSON` | The module updateJson URL (if set). |
| `KAM_STAGE` | Current build stage: `pre-build` or `post-build`. |
| `KAM_DEBUG` | Set to `1` to enable debug output in hooks. |
