# shellcheck shell=ash
# service.sh - 最小 wrapper
MODDIR=${0%/*}
[ -f "$MODDIR/lib/kamfw/.kamfwrc" ] && . "$MODDIR/lib/kamfw/.kamfwrc" || abort '! File "kamfw/.kamfwrc" does not exist!'

import __runtime__
kamfw run service -- "$@"
