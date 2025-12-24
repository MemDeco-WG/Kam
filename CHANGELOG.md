## v0.5.9 (2025-12-24)

## v0.5.8 (2025-12-16)

### Feat

- Implement i18n for CLI messages, add `install` command, and update workflows
- Introduce internationalization (i18n) support and enhance CLI output with improved tables and progress bars.
- Introduce `about` command, implement interactive `init` flow, and improve CI workflows with package manager caching.
- Add certificate and GitHub secret management, refactor signing by removing Fulcio/Sigstore integration, and update project version.
- Refactor CLI commands into handlers and args, and add new test environments and project templates.
- disable `kam sign` timestamping by default, requiring explicit opt-in.
- templates - make template cache path configurable (KAM_TEMPLATE_CACHE_DIR + tmpl.cache_dir), improve local template detection, and change default output filename to '{{id}}'; update docs and tests
- enhance hooks and documentation
- add automatic kam.toml to module.prop sync in pre-build hook
- improve template descriptions and convert ak3_template to proper template format
- add workspace glob pattern support and improve build exclusions

### Fix

- Suppress unused variable warnings by prefixing variables with an underscore.
- **pull**: follow redirects when downloading templates
- **template**: remove erroneous module.prop files from template source

### Refactor

- major code improvements and enhancements
