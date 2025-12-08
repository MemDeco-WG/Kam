#!/system/bin/sh
#
# action.sh
#
# This script is executed when the user clicks the "Action" button in the KernelSU Manager
# or triggers an action via the Module WebUI.
#
# ---------------------------------------------------------------------------------------
# EXECUTION CONTEXT
# ---------------------------------------------------------------------------------------
# - TRIGGER:      User interaction in KernelSU Manager (Action button) or WebUI.
# - ENV:          Runs in KernelSU's BusyBox ash shell (Standalone Mode).
#                 $MODDIR is set to the module's directory.
#                 $KSU_MODULE is set to the module ID.
# - OUTPUT:       Standard output (echo) is usually displayed to the user (e.g., as a Toast).
#
# ---------------------------------------------------------------------------------------
# REQUIREMENTS
# ---------------------------------------------------------------------------------------
# Minimum supported versions (required):
# - Magisk (stable): 28.0+
# - Magisk (alpha builds): alpha28001+ (e.g., 28001 or newer)
# - KernelSU kernel module: build 11986 / KernelSU v1.0.2+
# - (M/R)KernelSU (NEXT): build 12300+
#
# Notes:
# - These version constraints are required for KernelSU Manager (UI), KernelSU kernel
#   driver functionality, and the `ksud` utility that Module WebUI/Action scripts rely on.
# - 'alpha28001+' refers to Magisk alpha/canary builds and may be required for
#   certain alpha-only features. Test accordingly if you're using alpha builds.
# - '(M/R)KernelSU (NEXT)' refers to Main/Release NEXT builds of KernelSU where
#   newer manager and kernel build IDs are used.
# - If your module requires a higher version, add required runtime checks before
#   invoking version-specific features or APIs.
#
# ---------------------------------------------------------------------------------------
# MODULE CONFIGURATION (ksud)
# ---------------------------------------------------------------------------------------
# KernelSU provides a persistent key-value store for modules.
#
# Get value:      val=$(ksud module config get <key>)
# Set persist:    ksud module config set <key> <value>
# Set temp:       ksud module config set --temp <key> <value>
# Delete:         ksud module config delete <key>
# List all:       ksud module config list
#
# ---------------------------------------------------------------------------------------

MODDIR=${0%/*}

# ---------------------------------------------------------------------------------------
# EXAMPLE: Simple Feature Toggle
# ---------------------------------------------------------------------------------------
# This example toggles a feature flag and updates the module description.

# Read current state (default to false if empty)
# STATE=$(ksud module config get feature_enabled)
# [ -z "$STATE" ] && STATE="false"

# if [ "$STATE" = "true" ]; then
#     # Disable feature
#     ksud module config set feature_enabled "false"
#     ksud module config set override.description "Feature is currently DISABLED"
#     echo "Feature disabled"
# else
#     # Enable feature
#     ksud module config set feature_enabled "true"
#     ksud module config set override.description "Feature is currently ENABLED"
#     echo "Feature enabled"
# fi

# ---------------------------------------------------------------------------------------
# EXAMPLE: Managed Features
# ---------------------------------------------------------------------------------------
# Modules can control KernelSU internal features.
# Supported keys: manage.su_compat, manage.kernel_umount, manage.enhanced_security

# ksud module config set manage.su_compat true
# echo "Enforced SU Compatibility"

# ---------------------------------------------------------------------------------------
# DEFAULT ACTION
# ---------------------------------------------------------------------------------------

echo "Action script executed!"
echo "Edit action.sh to add custom logic."
