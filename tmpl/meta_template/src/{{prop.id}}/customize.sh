# shellcheck shell=ash

# {{prop.name}} customize.sh

ui_print "- Installing {{prop.name}}..."

# ---------------------------------------------------------
# 1. Environment Checks
# ---------------------------------------------------------

# Check if running in KernelSU
if [ "$KSU" = "true" ]; then
  ui_print "- Running in KernelSU environment"
  ui_print "- KernelSU Version: $KSU_VER ($KSU_VER_CODE)"
else
  ui_print "! Warning: Not running in KernelSU environment"
  # Uncomment to enforce KernelSU requirement
  # abort "! This module requires KernelSU"
fi

# Check for OverlayFS support
# Most metamodules rely on OverlayFS. It is highly recommended to check for it.
if grep -q "overlay" /proc/filesystems; then
  ui_print "- OverlayFS support detected"
else
  ui_print "! Warning: OverlayFS not found in /proc/filesystems"
  ui_print "! This module may not function correctly without OverlayFS support."
  # Uncomment to enforce OverlayFS requirement
  # abort "! OverlayFS is required"
fi

# ---------------------------------------------------------
# 2. Setup & Permissions
# ---------------------------------------------------------

# Set permissions for the module directory
ui_print "- Setting permissions..."
set_perm_recursive "$MODPATH" 0 0 0755 0644

# Ensure scripts are executable
# metamount.sh is critical for metamodules
if [ -f "$MODPATH/metamount.sh" ]; then
  set_perm "$MODPATH/metamount.sh" 0 0 0755
fi

if [ -f "$MODPATH/metainstall.sh" ]; then
  set_perm "$MODPATH/metainstall.sh" 0 0 0755
fi

if [ -f "$MODPATH/metauninstall.sh" ]; then
  set_perm "$MODPATH/metauninstall.sh" 0 0 0755
fi

# Standard lifecycle scripts
[ -f "$MODPATH/post-fs-data.sh" ] && set_perm "$MODPATH/post-fs-data.sh" 0 0 0755
[ -f "$MODPATH/service.sh" ] && set_perm "$MODPATH/service.sh" 0 0 0755
[ -f "$MODPATH/boot-completed.sh" ] && set_perm "$MODPATH/boot-completed.sh" 0 0 0755

# ---------------------------------------------------------
# 3. Custom Logic
# ---------------------------------------------------------

# Define files/folders to remove (whiteout)
# REMOVE="
# /system/app/SomeApp
# "

# Define folders to replace (opaque)
# REPLACE="
# /system/app/AnotherApp
# "

# Example: Create a persistent storage directory if needed
# PERSIST_DIR="/data/adb/{{prop.id}}"
# if [ ! -d "$PERSIST_DIR" ]; then
#   ui_print "- Creating persistent directory: $PERSIST_DIR"
#   mkdir -p "$PERSIST_DIR"
# fi

ui_print "- Installation complete"
