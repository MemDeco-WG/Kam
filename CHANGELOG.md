## Unreleased

## 0.4.20 (2025-12-11)

### Feat

- templates - make template cache path configurable (KAM_TEMPLATE_CACHE_DIR + tmpl.cache_dir), improve local template detection, and change default output filename to '{{id}}'; update docs and tests

### Fix

- **pull**: follow redirects when downloading templates

## 0.4.18 (2025-12-11)

## 0.4.16 (2025-12-10)

### Feat

- enhance hooks and documentation
- add automatic kam.toml to module.prop sync in pre-build hook
- improve template descriptions and convert ak3_template to proper template format
- add workspace glob pattern support and improve build exclusions

### Fix

- **template**: remove erroneous module.prop files from template source
