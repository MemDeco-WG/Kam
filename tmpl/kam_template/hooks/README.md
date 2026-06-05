# Kam Hook Overlay

Official hooks are restored by `kam init` from the remote base recorded in
`.kam/bases.toml` and live at `.kam/bases/hooks`.

Put project-specific hooks in this `hooks/` directory. Kam executes official
base hooks plus local overlay hooks. When both directories contain the same hook
filename for a stage, the local `hooks/<stage>/<file>` version overrides the
official base version.

Useful environment variables:

| Variable | Description |
|----------|-------------|
| `KAM_HOOKS_BASE_ROOT` | Official hook base, usually `.kam/bases/hooks`. |
| `KAM_HOOKS_ROOT` | Hook root for the script currently being executed. |
| `KAM_PROJECT_ROOT` | Project root directory. |
| `KAM_MODULE_ROOT` | Module source directory. |
| `KAM_DIST_DIR` | Build output directory. |
