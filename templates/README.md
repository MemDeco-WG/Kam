# Kam Templates

This directory contains template archives that can be imported into Kam's template cache.

## Available Templates

- **kam_template.tar.gz** (9.6MB) - Full-featured Kam module template with WebUI support
- **ak3_template.tar.gz** (3.1MB) - AnyKernel3 template for kernel modules
- **meta_template.tar.gz** (39KB) - Minimal metadata-only template

## Quick Start

### Import Templates

Import a single template:
```bash
kam tmpl import templates/meta_template.tar.gz
```

Import all templates at once (from a ZIP file):
```bash
# First, create a ZIP containing all templates
zip templates.zip *.tar.gz

# Then import
kam tmpl import templates.zip
```

### List Installed Templates

```bash
kam tmpl list
```

### Export Templates

Export a single template:
```bash
kam tmpl export meta_template -o my_template.tar.gz
```

Export multiple templates to a ZIP:
```bash
kam tmpl export kam_template ak3_template meta_template -o my_templates.zip
```

### Remove Templates

```bash
kam tmpl remove template_name
```

### Template Cache Location

View the template cache directory:
```bash
kam tmpl path
```

Default location: `~/.kam/templates/`

## Template Format

Templates are stored as `.tar.gz` archives containing:
- Module structure files
- Template variables (using Tera template syntax)
- Configuration files
- Scripts and assets

## Sharing Templates

To share templates with others:

1. **Single template**: Share the `.tar.gz` file directly
2. **Multiple templates**: Create a ZIP file containing multiple `.tar.gz` files

Example for sharing:
```bash
# Export your custom templates
kam tmpl export my_custom_template -o my_custom_template.tar.gz

# Share the my_custom_template.tar.gz file
```

## Creating Custom Templates

To create a custom template:

1. Initialize a new module project
2. Organize your files with template variables (e.g., `{{prop.id}}`, `{{prop.name}}`)
3. Create a `.tar.gz` archive
4. Import it: `kam tmpl import my_template.tar.gz --name my_custom`

## Notes

- Template names are automatically derived from filenames
- Use `--force` to overwrite existing templates
- The `tmpl_template` is embedded in the Kam binary as the default minimal template
- Large templates (like kam_template) are not embedded to keep the binary size small