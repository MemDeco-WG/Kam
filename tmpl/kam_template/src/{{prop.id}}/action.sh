#!/system/bin/sh
# shellcheck shell=ash
# action.sh - dispatch the action phase through kamfw.

MODDIR="${MODDIR:-${0%/*}}"
export MODDIR

# shellcheck disable=SC1090
. "$MODDIR/lib/kamfw/.kamfwrc" || exit 1
import __runtime__ || exit 1
kamfw run action -- "$@"
