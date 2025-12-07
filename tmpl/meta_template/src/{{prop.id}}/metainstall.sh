#!/system/bin/sh
#
# metainstall.sh - Metamodule Installation Hook
#
# This script is sourced by the KernelSU built-in installer when installing
# REGULAR modules (not when installing this metamodule itself).
#
# It allows you to customize the installation process of other modules.
#
# Execution Context:
# - This script is SOURCED, not executed directly.
# - It runs after files are extracted to $MODPATH but before the installation completes.

ui_print "*********************************************************"
ui_print "  Metamodule Hook: metainstall.sh"
ui_print "*********************************************************"

# ---------------------------------------------------------
# 1. Identify Metamodule
# ---------------------------------------------------------
# Export variables so other modules can detect they are being
# installed under this metamodule.
export KSU_HAS_METAMODULE="true"
export KSU_METAMODULE="{{prop.id}}"

# ---------------------------------------------------------
# 2. Override Partition Handling
# ---------------------------------------------------------
# By default, KernelSU moves partition folders (e.g., system/product -> product).
# If your metamodule mounting strategy prefers the standard hierarchy
# (keeping everything inside /system), you can override this function to no-op.
#
# Uncomment the function below if you want to preserve the original structure:

# handle_partition() {
#     # No-op: Prevent KernelSU from moving partition folders
#     echo 0 > /dev/null
#     return 0
# }

# ---------------------------------------------------------
# 3. Custom Logic
# ---------------------------------------------------------
# Add any other custom logic here (validation, file modification, etc.)

# ---------------------------------------------------------
# 4. Proceed with Installation
# ---------------------------------------------------------
# You MUST call install_module to let KernelSU finish the installation.
install_module
