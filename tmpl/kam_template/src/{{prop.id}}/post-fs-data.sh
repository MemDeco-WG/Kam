#!/system/bin/sh
# shellcheck shell=ash
# post-fs-data.sh - dispatch the post-fs-data phase through kamfw.

MODDIR="${MODDIR:-${0%/*}}"
export MODDIR

# shellcheck disable=SC1090
. "$MODDIR/lib/kamfw/.kamfwrc" || exit 1
import __runtime__ || exit 1
kamfw run post-fs-data -- "$@"
