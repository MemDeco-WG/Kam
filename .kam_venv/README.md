\<file\_path\>
Kam\\tmpl\\venv\_template\\README.md
\</file\_path\>

\<edit\_description\>
更新 venv\_template 的 README.md，使其更详细和专业
\</edit\_description\>

# Virtual Environment Template

## Description

This is a template for creating Kam virtual environment modules. Virtual environments provide isolated execution contexts for Kam modules, allowing them to run with their own set of dependencies and environment variables.

This template provides:

- Cross-platform activation scripts (Unix, Windows, PowerShell)
- Virtual environment directory structure
- Environment variable management
- Deactivation support
- Integration with Kam's venv system

## Usage

To create a new virtual environment module using this template:

1. Initialize a new project:
   
   ``` bash
   kam init my_venv --venv
   ```

2. Customize the environment:
   
   - Edit `kam.toml` for module metadata
   - Modify activation scripts as needed
   - Add custom executables to `bin/`
   - Add libraries to `lib/`

3. Build the module:
   
   ``` bash
   kam build
   ```

4. Use the virtual environment:
   
   ``` bash
   kam venv
   # Then activate:
   source .kam_venv/activate  # Unix
   # or
   .kam_venv\activate.bat     # Windows CMD
   # or
   .kam_venv\activate.ps1     # PowerShell
   ```

## Features

- **Cross-platform support**: Works on Unix-like systems, Windows CMD, and PowerShell
- **Environment isolation**: Maintains separate PATH and environment variables
- **Easy activation/deactivation**: Simple commands to enter/exit the environment
- **Customizable**: Easily extensible with additional scripts and variables
- **Kam integration**: Designed to work seamlessly with Kam's virtual environment system

## Structure

    my_venv/
    ├── kam.toml              # Module configuration
    ├── bin/                  # Executables and scripts
    ├── lib/                  # Libraries and dependencies
    ├── activate              # Unix activation script
    ├── activate.bat          # Windows CMD activation
    ├── activate.ps1          # PowerShell activation
    ├── activate.sh           # POSIX shell activation
    ├── deactivate            # Deactivation script
    ├── CHANGELOG.md          # Version history
    ├── LICENSE               # License information
    └── README.md             # This file

## Module Information

- **ID**: Kam
- **Name**: {{name}}
- **Version**: 1.0.0
- **Author**: Author
- **Type**: Template (Virtual Environment)

## Environment Variables

The activation scripts set the following environment variables:

- `KAM_OLD_PATH`: Backup of original PATH
- `PATH`: Modified to include venv bin directory
- `KAM_VENV_ACTIVE`: Marker indicating venv is active

## License

This template is provided under the MIT License. See LICENSE file for details.
