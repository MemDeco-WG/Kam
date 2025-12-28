## Unreleased

## v0.5.27 (2025-12-28)

## v0.5.26 (2025-12-28)

## v0.5.25 (2025-12-28)

## v0.5.24 (2025-12-27)

## v0.5.23 (2025-12-27)

## v0.5.22 (2025-12-27)

## v0.5.21 (2025-12-27)

## v0.5.20 (2025-12-27)

## v0.5.19 (2025-12-27)

## v0.5.18 (2025-12-26)

## v0.5.17 (2025-12-26)

## v0.5.16 (2025-12-26)

## v0.5.15 (2025-12-26)

## v0.5.14 (2025-12-26)

## v0.5.13 (2025-12-26)

## v0.5.12 (2025-12-26)

### Feat

- **install**: add Git repo install support; update CHANGELOG

### Fix

- **install**: convert dialoguer::Error into std::io::Error via Into

## v0.5.11 (2025-12-25)

## v0.5.10 (2025-12-24)

## v0.5.8 (2025-12-16)

## v0.5.6 (2025-12-16)

## v0.5.3 (2025-12-16)

## v0.4.38 (2025-12-15)

### Feat

- Implement i18n for CLI messages, add `install` command, and update workflows

## v0.4.37 (2025-12-14)

### Feat

- Introduce internationalization (i18n) support and enhance CLI output with improved tables and progress bars.
- Introduce `about` command, implement interactive `init` flow, and improve CI workflows with package manager caching.

### Refactor

- major code improvements and enhancements

## 0.4.31 (2025-12-13)

## 0.4.29 (2025-12-12)

## 0.4.28 (2025-12-12)

## 0.4.27 (2025-12-12)

### Feat

- Add certificate and GitHub secret management, refactor signing by removing Fulcio/Sigstore integration, and update project version.

## 0.4.24 (2025-12-12)

### Feat

- Refactor CLI commands into handlers and args, and add new test environments and project templates.
- disable `kam sign` timestamping by default, requiring explicit opt-in.

### Fix

- Suppress unused variable warnings by prefixing variables with an underscore.

## 0.4.22 (2025-12-12)

## 0.4.21 (2025-12-12)

## 0.4.20 (2025-12-11)

### Feat

- templates - make template cache path configurable (KAM_TEMPLATE_CACHE_DIR + tmpl.cache_dir), improve local template detection, and change default output filename to '{{id}}'; update docs and tests

### Fix

- **pull**: follow redirects when downloading templates

## 0.4.18 (2025-12-11)

## 0.4.16 (2025-12-10)

## 0.4.13 (2025-12-10)

## 0.4.12 (2025-12-09)

## 0.4.11 (2025-12-08)

## 0.4.3 (2025-12-08)

## 0.4.2 (2025-12-08)

## 0.4.1 (2025-12-08)

## 0.3.8 (2025-12-07)

## 0.3.7 (2025-12-07)

## 0.3.3 (2025-12-07)

## 0.3.2 (2025-12-07)

### Fix

- **template**: remove erroneous module.prop files from template source

## 0.3.1 (2025-12-07)

### Feat

- enhance hooks and documentation

## 0.3.0 (2025-12-07)

### Feat

- add automatic kam.toml to module.prop sync in pre-build hook

## 0.2.0 (2025-12-07)

### Feat

- improve template descriptions and convert ak3_template to proper template format
- add workspace glob pattern support and improve build exclusions
