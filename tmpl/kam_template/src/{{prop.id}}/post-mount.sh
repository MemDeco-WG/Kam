# shellcheck shell=ash
# post-mount.sh
#
# This script runs in the "post-mount" stage of the boot process.
#
# ---------------------------------------------------------------------------------------
# EXECUTION CONTEXT
# ---------------------------------------------------------------------------------------
# - TIMING:       Runs AFTER the module's system directory has been mounted (OverlayFS).
#                 Runs BEFORE the "service" stage.
# - ENV:          Runs in KernelSU's BusyBox ash shell (Standalone Mode).
#                 $MODDIR is set to the module's directory.
#                 $KSU_MODULE is set to the module ID.
#
# ---------------------------------------------------------------------------------------
# USE CASES
# ---------------------------------------------------------------------------------------
# - Operations that depend on the module's files being visible in /system.
# - Verifying that mounts were successful.
# - Modifying files that were just mounted (though usually better done in post-fs-data).
# - Interacting with other modules that might have just been mounted.
#
# ---------------------------------------------------------------------------------------

MODDIR=${0%/*}
[ -f "$MODDIR/lib/kamfw/.kamfwrc" ] && . "$MODDIR/lib/kamfw/.kamfwrc" || abort '! File "kamfw/.kamfwrc" does not exist!'
