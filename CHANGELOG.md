## Unreleased

### Feat

- Introduce `about` command, implement interactive `init` flow, and improve CI workflows with package manager caching.

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
