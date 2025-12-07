#!/system/bin/sh

# {{prop.name}} customize.sh
#
# This script is sourced by the module installer script after all files are extracted
# and default permissions/secontext are applied.
#
# Useful for:
# - Checking device compatibility (ARCH, API)
# - Setting special permissions
# - Customizing installation based on user environment
#
# ---------------------------------------------------------------------------------------
# AVAILABLE VARIABLES
# ---------------------------------------------------------------------------------------
# KSU (bool):           true if running in KernelSU environment
# KSU_VER (string):     KernelSU version string (e.g. v0.9.5)
# KSU_VER_CODE (int):   KernelSU version code (userspace)
# KSU_KERNEL_VER_CODE (int): KernelSU version code (kernel space)
# BOOTMODE (bool):      always true in KernelSU
# MODPATH (path):       Path where module files are installed (e.g. /data/adb/modules/{{prop.id}})
# TMPDIR (path):        Path to temporary directory
# ZIPFILE (path):       Path to the installation ZIP
# ARCH (string):        Device architecture: arm, arm64, x86, x64
# IS64BIT (bool):       true if ARCH is arm64 or x64
# API (int):            Android API level (e.g. 33 for Android 13)
#
# ---------------------------------------------------------------------------------------
# AVAILABLE FUNCTIONS
# ---------------------------------------------------------------------------------------
# ui_print <msg>
#     Print message to console. Avoid 'echo'.
#
# abort <msg>
#     Print error message and terminate installation.
#
# set_perm <target> <owner> <group> <permission> [context]
#     Set permissions for a file.
#     Default context: "u:object_r:system_file:s0"
#
# set_perm_recursive <dir> <owner> <group> <dirperm> <fileperm> [context]
#     Recursively set permissions for a directory.
#     Default context: "u:object_r:system_file:s0"
#
# ---------------------------------------------------------------------------------------
# KERNELSU FEATURES
# ---------------------------------------------------------------------------------------
#
# REMOVE (Whiteout):
# List directories/files to be "removed" from the system (overlaid with whiteout).
# KernelSU executes: mknod <TARGET> c 0 0
#
# REMOVE="
# /system/app/BloatwareApp
# /system/priv-app/AnotherApp
# "
#
# REPLACE (Opaque):
# List directories to be replaced by an empty directory (or your module's version).
# KernelSU executes: setfattr -n trusted.overlay.opaque -v y <TARGET>
#
# REPLACE="
# /system/app/YouTube
# "
#
# ---------------------------------------------------------------------------------------
# CUSTOM INSTALLATION LOGIC
# ---------------------------------------------------------------------------------------

ui_print "- Installing {{prop.name}}..."

# Check environment
if [ "$KSU" = "true" ]; then
  ui_print "- Running in KernelSU environment"
  ui_print "- KernelSU Version: $KSU_VER ($KSU_VER_CODE)"
else
  ui_print "- Running in Magisk/Other environment"
fi

# Example: Check Android Version
# if [ "$API" -lt 26 ]; then
#   abort "! Android 8.0+ required"
# fi

# Example: Check Architecture
# if [ "$ARCH" != "arm64" ]; then
#   abort "! Only arm64 is supported"
# fi

# Set permissions
# Default permissions are usually sufficient, but you can be explicit.
ui_print "- Setting permissions..."
set_perm_recursive "$MODPATH" 0 0 0755 0644

# If you have scripts, make them executable
# set_perm "$MODPATH/service.sh" 0 0 0755
# set_perm "$MODPATH/post-fs-data.sh" 0 0 0755
# set_perm "$MODPATH/action.sh" 0 0 0755

# ---------------------------------------------------------------------------------------
# FULL CONTROL (SKIPUNZIP)
# ---------------------------------------------------------------------------------------
# If you want to handle extraction manually, uncomment the line below.
# SKIPUNZIP=1
#
# If SKIPUNZIP=1 is set, you must extract files yourself:
# unzip -o "$ZIPFILE" -x 'META-INF/*' -d "$MODPATH" >&2
