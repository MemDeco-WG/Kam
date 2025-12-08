#!/bin/sh
#
# action.sh
#
# Simple script wrapping `kam` commands; used by the project for quick tasks and diagnostics.
#
# ---------------------------------------------------------------------------------------
# REQUIREMENTS (最低版本要求)
# ---------------------------------------------------------------------------------------
# - Magisk (stable)                : 28.0+
# - Magisk (alpha build)           : alpha28001+
# - KernelSU (Manager / Runtime)   : 11986 / 1.0.2+
# - (M/R)KernelSU (NEXT)           : 12300+
#
# Notes:
# - These are minimum required versions to ensure that the module's Action features,
#   the Module WebUI and KernelSU's `ksud` utilities (key-value store) are available
#   and behave as expected. If your environment doesn't meet these versions, some
#   functionality may be limited or fail.
# - 'alpha28001+' refers to Magisk alpha/canary builds and may be required for
#   alpha-only features. Test accordingly if using alpha builds.
#
# 注意：
# - Magisk（稳定版）: 28.0 及以上
# - Magisk（Alpha 构建）: alpha28001 及以上
# - KernelSU（管理器/运行时）: 11986 / 1.0.2 及以上
# - (M/R)KernelSU（NEXT）: 12300 及以上
#
# ---------------------------------------------------------------------------------------
kam --help

ui_print "tmpl help:"
kam tmpl help

ui_print "tmpl list:"
kam tmpl list

ui_print "cache path:"
kam cache path

ui_print "kam version:"
kam --version
