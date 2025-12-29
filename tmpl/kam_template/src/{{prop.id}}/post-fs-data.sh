# shellcheck shell=ash
# post-fs-data.sh
MODDIR=${0%/*}
[ -f "$MODDIR/lib/kamfw/.kamfwrc" ] && . "$MODDIR/lib/kamfw/.kamfwrc" || abort '! File "kamfw/.kamfwrc" does not exist!'
# 🚨中文提示：一般情况不需要这个脚本
# 如果你不了解，你只需要记得这个脚本执行时机很早就行了
# This script runs in the "post-fs-data" stage of the boot process.
#
# ---------------------------------------------------------------------------------------
# EXECUTION CONTEXT
# ---------------------------------------------------------------------------------------
# - BLOCKING: The boot process is PAUSED until this script finishes (or times out).
# - TIMEOUT:  Usually 10 seconds. If it takes longer, boot continues.
# - TIMING:   Runs BEFORE modules are mounted.
# - ENV:      Runs in KernelSU's BusyBox ash shell (Standalone Mode).
#             $MODDIR is set to the module's directory.
#             $KSU_MODULE is set to the module ID.
#
# ---------------------------------------------------------------------------------------
# USE CASES
# ---------------------------------------------------------------------------------------
# - Dynamically modifying module files before they are mounted.
# - Loading custom sepolicy rules (if not using sepolicy.rule file).
# - Setting system properties (use `resetprop`).
# - Managing module configuration (clearing temp configs).
#
# ---------------------------------------------------------------------------------------
# MODULE CONFIGURATION (KernelSU)
# ---------------------------------------------------------------------------------------
# KernelSU provides a built-in key-value store for modules.
#
# Get a value:
# val=$(ksud module config get my_key)
#
# Set a persistent value:
# ksud module config set my_key "value"
#
# Set a temporary value (cleared on next boot):
# ksud module config set --temp runtime_state "active"
#
# ---------------------------------------------------------------------------------------
import __runtime__
# export KAM_LOGFILE=${MODDIR}/kam.log
