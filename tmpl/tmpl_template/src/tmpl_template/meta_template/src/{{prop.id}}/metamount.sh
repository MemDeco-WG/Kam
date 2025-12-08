#!/system/bin/sh
#
# metamount.sh - Metamodule Mount Handler
#
# This script is executed by KernelSU during the boot process to handle
# the mounting of regular modules.
#
# Execution Order:
# 1. post-fs-data scripts
# 2. metamount.sh (THIS SCRIPT)
# 3. post-mount scripts
#
# CRITICAL REQUIREMENT:
# Any mount operation performed here MUST identify the source as "KSU".
# For 'mount' command, use '-o dev=KSU' (if supported) or ensure the device argument is "KSU".
# Failure to do so may prevent KernelSU from tracking the mount properly.
#
# NOTE: This script runs during boot. 'ui_print' is NOT available here.
# We log to /dev/kmsg for debugging (viewable via 'dmesg').

MODDIR="${0%/*}"
MODULES_DIR="/data/adb/modules"
DMESG_PREFIX="[{{prop.id}}/metamount]"

# ---------------------------------------------------------
# 1. Single Instance Check
# ---------------------------------------------------------
# Prevent the script from running multiple times if KSU triggers it redundantly.
LOCK_FILE="/dev/{{prop.id}}_metamount_lock"
if [ -f "$LOCK_FILE" ]; then
    echo "$DMESG_PREFIX Already ran, skipping." > /dev/kmsg
    exit 0
fi
touch "$LOCK_FILE"

echo "$DMESG_PREFIX Initializing mount handler..." > /dev/kmsg

# ---------------------------------------------------------
# 2. Environment Setup
# ---------------------------------------------------------
if [ ! -d "$MODULES_DIR" ]; then
    echo "$DMESG_PREFIX No modules directory found at $MODULES_DIR" > /dev/kmsg
    exit 0
fi

# ---------------------------------------------------------
# 3. Module Processing
# ---------------------------------------------------------

process_module() {
    local module_path="$1"
    local module_id="${module_path##*/}"

    # Skip if not a directory
    if [ ! -d "$module_path" ]; then
        return
    fi

    # Skip the metamodule itself
    if [ "$module_path" = "$MODDIR" ]; then
        return
    fi

    # Skip if disabled
    if [ -f "$module_path/disable" ]; then
        echo "$DMESG_PREFIX Skipping disabled module: $module_id" > /dev/kmsg
        return
    fi

    # Skip if module requests to skip mount
    if [ -f "$module_path/skip_mount" ]; then
        echo "$DMESG_PREFIX Skipping module (skip_mount): $module_id" > /dev/kmsg
        return
    fi

    # Skip if module requests to skip this specific metamodule (optional convention)
    if [ -f "$module_path/skip_{{prop.id}}" ]; then
        echo "$DMESG_PREFIX Skipping module (explicit skip): $module_id" > /dev/kmsg
        return
    fi

    echo "$DMESG_PREFIX Processing module: $module_id" > /dev/kmsg

    # ---------------------------------------------------------
    # MOUNTING LOGIC
    # ---------------------------------------------------------
    # Example: Bind mounting a 'system' directory

    if [ -d "$module_path/system" ]; then
        # Syntax for Bind Mount:
        # mount -o bind,dev=KSU "/source/path" "/target/path"

        # Syntax for OverlayFS:
        # mount -t overlay -o lowerdir="/lower",upperdir="/upper",workdir="/work",dev=KSU KSU "/target"

        echo "$DMESG_PREFIX   - Found system directory (ready to mount)" > /dev/kmsg

        # Actual implementation would go here.
        # For a template, we leave this as a placeholder or a simple example.
    fi
}

# Iterate through all modules
for module in "$MODULES_DIR"/*; do
    process_module "$module"
done

echo "$DMESG_PREFIX Mount handler completed." > /dev/kmsg
