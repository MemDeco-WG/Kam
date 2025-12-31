#!/system/bin/sh
# shellcheck shell=ash
# action.sh - minimal wrapper (RECTIFY-FINAL)

MODDIR=${0%/*}

# shellcheck disable=SC1090
. "$MODDIR/lib/kamfw/.kamfwrc" || exit 1

print "[kamfw] phase=action args=$*"

exit 0
