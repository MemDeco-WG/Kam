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
   - Modify `src/test_module_var/customize.sh` for installation logic
   - Add module files to `src/test_module_var/`

3. Build the module:
   
   ``` bash
   kam build
   ```

## Module Information

- **ID**: test_module_var
- **Name**: test_module_var
- **Version**: 0.1.0
- **Author**: Author

## License

This template is provided under the MIT License. See LICENSE file for details.
