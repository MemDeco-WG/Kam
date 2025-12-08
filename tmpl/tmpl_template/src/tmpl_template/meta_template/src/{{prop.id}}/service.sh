#!/system/bin/sh
#
# service.sh - Metamodule Late Start Service Script
#
# This script runs during the late_start service stage of the boot process.
# It is executed after the system has largely booted up.
#
# Execution Order:
# 1. Common service.d scripts
# 2. THIS SCRIPT (Metamodule service.sh)
# 3. Regular modules' service.sh scripts
#
# Use this script for background tasks, long-running processes, or
# operations that depend on the system being fully initialized.

MODDIR="${0%/*}"

echo "[{{prop.id}}] Executing service.sh..." > /dev/kmsg

# ---------------------------------------------------------
# YOUR LOGIC GOES HERE
# ---------------------------------------------------------

# Example: Wait for boot completion before doing something
# until [ "$(getprop sys.boot_completed)" = "1" ]; do
#     sleep 1
# done
# echo "[{{prop.id}}] Boot completed, starting background service..." > /dev/kmsg

# Example: Start a daemon or background process
# nohup "$MODDIR/my_daemon" > /dev/null 2>&1 &

# ---------------------------------------------------------

echo "[{{prop.id}}] service.sh completed." > /dev/kmsg
