#!/system/bin/sh
# shellcheck shell=ash
# customize.sh - minimal wrapper (RECTIFY-FINAL)

SKIPUNZIP=1

# 安装期解压框架（关键路径：失败必须 abort，禁止 silent fail）
unzip -o "$ZIPFILE" "lib/kamfw/*" -d "$MODPATH" >&2 || abort "! Failed to extract lib/kamfw"

export MODDIR="${MODDIR:-$MODPATH}"

# shellcheck disable=SC1090
. "$MODDIR/lib/kamfw/.kamfwrc" || abort "! failed to source .kamfwrc"

print "[kamfw] phase=install"

exit 0
