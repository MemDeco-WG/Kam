# Kam Module Template

## Description

This is a template for creating Kam modules. Kam modules are custom modifications for Android devices, typically installed via Magisk or similar managers.

This template provides a basic structure for a Kam module, including:

- Module metadata and configuration
- Installation scripts
- Basic file structure

## Usage

### Quick Start

1. Initialize a new project:
   
   ``` bash
   kam init my_module --kam
   ```

2. Customize the module:
   
   - Edit `kam.toml` for module metadata
   - Modify `src/Kam/customize.sh` for installation logic
   - Add module files to `src/Kam/`

3. Build the module:
   
   ``` bash
   kam build
   ```

### Template Management

Kam supports importing and exporting module templates for easy sharing and reuse.

#### Import Templates

Import a single template:
```bash
kam tmpl import templates/meta_template.tar.gz
```

Import multiple templates from a ZIP file:
```bash
kam tmpl import templates/all-templates.zip
```

#### List Available Templates

```bash
kam tmpl list
```

#### Export Templates

Export a single template:
```bash
kam tmpl export meta_template -o my_template.tar.gz
```

Export multiple templates to a ZIP:
```bash
kam tmpl export kam_template ak3_template -o my_templates.zip
```

#### Additional Template Commands

```bash
# Remove a template from cache
kam tmpl remove template_name

# Show template cache directory
kam tmpl path
```

For more details on templates, see [templates/README.md](templates/README.md).

## Module Information

- **ID**: Kam
- **Name**: Kam
- **Version**: 0.1.0
- **Author**: Author

## License

This template is provided under the MIT License. See LICENSE file for details.
