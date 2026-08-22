#!/system/bin/sh
# shellcheck shell=ash
# boot-completed.sh - dispatch the boot-completed phase through kamfw.

MODDIR="${MODDIR:-${0%/*}}"
export MODDIR

# shellcheck disable=SC1090
. "$MODDIR/lib/kamfw/.kamfwrc" || exit 1
import __runtime__ || exit 1
kamfw run boot-completed -- "$@"
