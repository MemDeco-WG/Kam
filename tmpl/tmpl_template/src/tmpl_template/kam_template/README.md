# {{project_name}}

{{description}}

This is a **KernelSU Module** project created with [Kam](https://github.com/MemDeco-WG/Kam).

## ⚠️ Important: System Modification

**KernelSU uses a metamodule architecture.**

If this module contains files in the `system/` directory intended to modify the system partition (via OverlayFS), users **MUST** have a compatible **Metamodule** (like `meta-overlayfs`) installed. Without a metamodule, the `system/` directory in this module will be ignored, although scripts (`service.sh`, etc.) will still run.

## Project Structure

```text
.
├── kam.toml                 # Project configuration
├── src/
│   └── {{prop.id}}/         # Module source code
│       ├── module.prop      # Generated automatically
│       ├── system/          # Files to be mounted (requires Metamodule)
│       ├── customize.sh     # Installation logic
│       ├── post-fs-data.sh  # Blocking boot script
│       ├── service.sh       # Non-blocking boot script
│       ├── boot-completed.sh# Runs after boot complete
│       ├── action.sh        # Action button handler
│       ├── system.prop      # System properties
│       └── sepolicy.rule    # SELinux rules
└── hooks/                   # Build hooks
```

## Lifecycle Scripts

This template includes standard KernelSU lifecycle scripts. They run in KernelSU's built-in BusyBox `ash` shell (Standalone Mode).

1.  **`post-fs-data.sh`** (Blocking)
    *   Runs before Zygote starts.
    *   **Blocking:** Pauses boot for up to 10 seconds.
    *   Use for: Dynamic file modification, loading sepolicy, resetting props.
    *   *Note: Do not use `setprop` here (deadlock risk); use `resetprop -n`.*

2.  **`post-mount.sh`** (Blocking)
    *   Runs after the module's `system/` directory has been mounted (by the metamodule).
    *   Use for: Verifying mounts, operations depending on overlaid files.

3.  **`service.sh`** (Non-blocking)
    *   Runs in parallel with boot (late_start).
    *   Use for: Background daemons, long-running tasks.

4.  **`boot-completed.sh`**
    *   Runs when `sys.boot_completed` is "1".
    *   Use for: UI interactions, notifications, final cleanup.

## Module Configuration

KernelSU provides a built-in key-value store for modules, accessible via the `ksud` command in your scripts.

```bash
# Set a persistent value (survives reboot)
ksud module config set my_setting "value"

# Set a temporary value (cleared on reboot)
ksud module config set --temp runtime_state "active"

# Get a value
value=$(ksud module config get my_setting)
```

**Advanced Config Features:**
*   **Override Description:** `ksud module config set override.description "New Status"`
*   **Manage Features:** `ksud module config set manage.su_compat true`

## Installation Customization

The `customize.sh` script runs during installation.

*   **Permissions:** Use `set_perm` and `set_perm_recursive` to set file permissions.
*   **Removal (Whiteout):** Define `REMOVE` variable in `customize.sh` to hide system files.
*   **Replacement (Opaque):** Define `REPLACE` variable in `customize.sh` to replace system directories.

## WebUI & Action

*   **`action.sh`**: Executed when the user clicks the "Action" button in the KernelSU Manager or via WebUI.
*   **WebUI**: You can add a web-based interface in `src/{{prop.id}}/webroot/` (if supported by your template setup).

## Building

Run the following command to build your module:

```bash
kam build
```

The output zip file will be located in `dist/` (or the target directory specified in `kam.toml`).

## Installation

1.  Open KernelSU Manager.
2.  Install the generated ZIP file.
3.  Reboot.