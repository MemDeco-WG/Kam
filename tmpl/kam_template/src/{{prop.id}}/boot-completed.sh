# shellcheck shell=ash
# boot-completed.sh - 最小 wrapper
MODDIR=${0%/*}
[ -f "$MODDIR/lib/kamfw/.kamfwrc" ] && . "$MODDIR/lib/kamfw/.kamfwrc" || abort '! File "kamfw/.kamfwrc" does not exist!'

import __runtime__
kamfw run boot-completed -- "$@"
