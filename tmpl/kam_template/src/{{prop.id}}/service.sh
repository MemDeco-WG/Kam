# shellcheck shell=ash
# service.sh
MODDIR=${0%/*}
[ -f "$MODDIR/lib/kamfw/.kamfwrc" ] && . "$MODDIR/lib/kamfw/.kamfwrc" || abort '! File "kamfw/.kamfwrc" does not exist!'
# This script runs in the "late_start service" stage of the boot process.
#
# ---------------------------------------------------------------------------------------
# EXECUTION CONTEXT
# ---------------------------------------------------------------------------------------
# - NON-BLOCKING: Runs in parallel with the rest of the boot process.
# - TIMING:       Runs after the system is up and modules are mounted.
# - ENV:          Runs in KernelSU's BusyBox ash shell (Standalone Mode).
#                 $MODDIR is set to the module's directory.
#                 $KSU_MODULE is set to the module ID.
#
# ---------------------------------------------------------------------------------------
# USE CASES
# ---------------------------------------------------------------------------------------
# - Starting background daemons or services.
# - Running tasks that take a long time and shouldn't delay boot.
# - Operations that require the system to be fully initialized.
# - Waiting for specific system properties (like sys.boot_completed).
#
# ---------------------------------------------------------------------------------------
import __runtime__
