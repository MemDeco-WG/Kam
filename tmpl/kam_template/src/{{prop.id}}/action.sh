# shellcheck shell=ash
# action.sh
#
# 🚨 模块卡片按钮点击时执行，需新版
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
# 🚨 版本要求
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
# 🚨 仅限新版本
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
[ -f "$MODDIR/lib/kamfw/.kamfwrc" ] && . "$MODDIR/lib/kamfw/.kamfwrc" || abort '! File "kamfw/.kamfwrc" does not exist!'
import __runtime__
