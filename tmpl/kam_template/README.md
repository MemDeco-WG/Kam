\<file\_path\>
Kam\\tmpl\\kam\_template\\README.md
\</file\_path\>

\<edit\_description\>
更新 kam\_template 的 README.md，使其更详细和专业
\</edit\_description\>

# Kam Module Template

## Description

This is a template for creating Kam modules. Kam modules are custom modifications for Android devices, typically installed via Magisk or similar managers.

This template provides a basic structure for a Kam module, including:

- Module metadata and configuration
- Installation scripts
- Basic file structure

## Usage

To create a new Kam module using this template:

1. Initialize a new project:
   
   ``` bash
   kam init my_module --kam
   ```

2. Customize the module:
   
   - Edit `kam.toml` for module metadata
   - Modify `src/{{id}}/customize.sh` for installation logic
   - Add module files to `src/{{id}}/`

3. Build the module:
   
   ``` bash
   kam build
   ```

## Module Information

- **ID**: {{id}}
- **Name**: {{name}}
- **Version**: 0.1.0
- **Author**: Author

## License

This template is provided under the MIT License. See LICENSE file for details.
