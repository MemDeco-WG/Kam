# shellcheck shell=ash
# action.sh - 最小 wrapper（必须支持参数透传）
MODDIR=${0%/*}
[ -f "$MODDIR/lib/kamfw/.kamfwrc" ] && . "$MODDIR/lib/kamfw/.kamfwrc" || abort '! File "kamfw/.kamfwrc" does not exist!'

import __runtime__
kamfw run action -- "$@"
