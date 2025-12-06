# Template Module Template

## Description

This is a template for creating Kam template modules. Template modules define reusable project structures and configurations that can be used to initialize new Kam projects.

This template provides:

- Template metadata and configuration
- Variable definitions for customization
- Basic template structure
- Example customization script

## Usage

To create a new template module using this template:

1. Initialize a new project:

   ``` bash
   kam init my_template --tmpl
   ```

2. Customize the template:

   - Edit `kam.toml` for template metadata and variables
   - Modify `src/ak3_template/customize.sh` for template-specific logic
   - Add template files and placeholders

3. Build the template:

   ``` bash
   kam build
   ```

## Template Variables

This template supports the following variables (defined in `kam.toml`):

- `example`: A required string variable for demonstration

## Module Information

- **ID**: ak3_template
- **Name**: ak3_template
- **Version**: 1.0.0
- **Author**: Your Name
- **Type**: Template

## License

This template is provided under the MIT License. See LICENSE file for details.
