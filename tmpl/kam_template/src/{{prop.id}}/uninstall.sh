#!/system/bin/sh
# shellcheck shell=ash
# uninstall.sh - minimal wrapper (RECTIFY-FINAL)

MODDIR=${0%/*}

# shellcheck disable=SC1090
. "$MODDIR/lib/kamfw/.kamfwrc" || exit 1

print "[kamfw] phase=uninstall"

exit 0
