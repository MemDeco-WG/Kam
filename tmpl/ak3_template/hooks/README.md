### Environment Variables

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
| `KAM_STAGE` | Current build stage: `pre-build` or `post-build`. |
