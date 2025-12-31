#!/system/bin/sh
# shellcheck shell=ash
# post-fs-data.sh - minimal wrapper (RECTIFY-FINAL)

MODDIR=${0%/*}

# 第一行有效代码必须 source .kamfwrc（提供 print/ui_print/abort 等）
# shellcheck disable=SC1090
. "$MODDIR/lib/kamfw/.kamfwrc" || exit 1

# 业务 phase 占位：后续可改为调度器
print "[kamfw] phase=post-fs-data"

exit 0
