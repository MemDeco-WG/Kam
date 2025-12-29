# shellcheck shell=ash
#
# metauninstall.sh - Metamodule Cleanup Hook
#
# This script is executed by KernelSU when a REGULAR module is uninstalled.
# It allows the metamodule to clean up any resources associated with the
# uninstalled module.
#
# Execution Context:
# - Runs before the module directory is removed.
#
# Arguments:
# $1: MODULE_ID - The ID of the module being uninstalled
#

MODULE_ID="$1"
DMESG_PREFIX="[{{prop.id}}/metauninstall]"

# ---------------------------------------------------------
# 1. Validation
# ---------------------------------------------------------
if [ -z "$MODULE_ID" ]; then
    echo "$DMESG_PREFIX Error: Called without MODULE_ID" > /dev/kmsg
    exit 1
fi

echo "$DMESG_PREFIX Cleaning up resources for module: $MODULE_ID" > /dev/kmsg

# ---------------------------------------------------------
# 2. Cleanup Logic
# ---------------------------------------------------------

# Example: If your metamodule creates temporary flags or files inside the module directory
# (like skip_mount), they will be deleted automatically when the module dir is removed.
# However, if you track state in a separate persistent directory, you must clean it up here.

# PERSIST_DIR="/data/adb/{{prop.id}}"
# MODULE_STATE_FILE="$PERSIST_DIR/states/$MODULE_ID"

# if [ -f "$MODULE_STATE_FILE" ]; then
#     echo "$DMESG_PREFIX Removing state file for $MODULE_ID" > /dev/kmsg
#     rm -f "$MODULE_STATE_FILE"
# fi

# Example: If you use a separate image or mount point for each module
# IMG_MNT="/data/adb/{{prop.id}}/mnt/$MODULE_ID"
# if [ -d "$IMG_MNT" ]; then
#     echo "$DMESG_PREFIX Removing mount directory for $MODULE_ID" > /dev/kmsg
#     rm -rf "$IMG_MNT"
# fi

# ---------------------------------------------------------
# 3. Completion
# ---------------------------------------------------------

echo "$DMESG_PREFIX Cleanup completed for $MODULE_ID" > /dev/kmsg
