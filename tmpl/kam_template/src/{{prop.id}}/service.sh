#!/system/bin/sh
# shellcheck shell=ash
# service.sh - minimal wrapper (RECTIFY-FINAL)

MODDIR=${0%/*}

# shellcheck disable=SC1090
. "$MODDIR/lib/kamfw/.kamfwrc" || exit 1

print "[kamfw] phase=service"

exit 0
