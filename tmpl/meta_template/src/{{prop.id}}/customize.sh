#!/system/bin/sh

# {{prop.name}} customize.sh

ui_print "- Installing {{prop.name}}..."

# Check if running in KernelSU
if [ "$KSU" = "true" ]; then
  ui_print "- Running in KernelSU environment"
  ui_print "- KernelSU Version: $KSU_VER ($KSU_VER_CODE)"
else
  ui_print "! Warning: Not running in KernelSU environment"
fi

# Metamodule configuration
# Define files/folders to remove (whiteout)
# REMOVE="
# /system/app/SomeApp
# "

# Define folders to replace (opaque)
# REPLACE="
# /system/app/AnotherApp
# "

# Set permissions
set_perm_recursive "$MODPATH" 0 0 0755 0644

# Custom installation logic goes here
