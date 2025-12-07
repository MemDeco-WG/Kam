# {{project_name}}

{{description}}

This is a **KernelSU Metamodule** project created with [Kam](https://github.com/MemDeco-WG/Kam).

## What is a Metamodule?

A metamodule is a special type of KernelSU module that controls how *other* regular modules are installed and mounted. It replaces the internal mounting logic of KernelSU, allowing for complete customization of the module system (e.g., using OverlayFS, Magic Mount, etc.).

**Note:** Only one metamodule can be installed at a time.

## Project Structure

```text
.
├── kam.toml                 # Project configuration
├── src/
│   └── {{prop.id}}/         # Module source code
│       ├── module.prop      # Generated automatically
│       ├── metamount.sh     # [CRITICAL] Mount handler
│       ├── metainstall.sh   # [OPTIONAL] Installation hook for other modules
│       ├── metauninstall.sh # [OPTIONAL] Cleanup hook for other modules
│       ├── post-fs-data.sh  # Standard lifecycle script
│       ├── service.sh       # Standard lifecycle script
│       └── ...
└── hooks/                   # Build hooks (pre/post build scripts)
```

## Metamodule Hooks

This template includes the three special hooks available to metamodules.

### 1. `metamount.sh` (Mount Handler)
This is the most important script. It runs during boot and is responsible for mounting all regular modules.

*   **Execution:** Runs after `post-fs-data` but before `post-mount`.
*   **Responsibility:** Iterate through `/data/adb/modules`, check for `disable`/`skip_mount` flags, and mount module files into the system.
*   **CRITICAL:** You **MUST** set the mount source/device to `KSU` (e.g., `mount -o bind,dev=KSU ...`). This allows KernelSU to track the mounts.

### 2. `metainstall.sh` (Installation Hook)
This script is **sourced** by the KernelSU installer when a user installs a *regular* module.

*   **Execution:** During module installation (after extraction, before finalization).
*   **Responsibility:** Validate module compatibility, modify module files before they are installed, or prepare custom storage.
*   **Note:** This does NOT run when installing the metamodule itself.

### 3. `metauninstall.sh` (Cleanup Hook)
This script runs when a regular module is uninstalled.

*   **Execution:** Before the module directory is removed.
*   **Responsibility:** Clean up any resources (like separate images or databases) associated with the module being removed.

## Lifecycle Scripts

Metamodules also support standard KernelSU lifecycle scripts, which run **before** regular modules:

1.  `post-fs-data.sh` (Metamodule) -> `post-fs-data.sh` (Regular Modules)
2.  `metamount.sh` (Metamodule)
3.  `service.sh` (Metamodule) -> `service.sh` (Regular Modules)
4.  `boot-completed.sh` (Metamodule) -> `boot-completed.sh` (Regular Modules)

## Configuration

Edit `kam.toml` to configure your project.

*   **[prop]**: Basic module metadata (id, name, version, author).
    *   `metamodule = true` is set automatically for this template.
*   **[mmrl.repo]**: Metadata for MMRL/module repositories.
*   **[kam.build]**: Build settings (output directory, excludes).

## Building

Run the following command to build your metamodule:

```bash
kam build
```

The output zip file will be located in `dist/` (or the target directory specified in `kam.toml`).

## Installation

1.  Open KernelSU Manager.
2.  Install the generated ZIP file like a regular module.
3.  Reboot.

**Warning:** Installing a metamodule will replace any existing metamodule. Ensure you have a backup or know how to restore if boot issues occur.