#!/system/bin/sh
# shellcheck shell=ash
# customize.sh - install the module and dispatch the kamfw install phase.

SKIPUNZIP=1

# Extract the framework before loading it; all other files go through the
# installer API so filters and executable modes stay consistent.
unzip -o "$ZIPFILE" "lib/kamfw/*" ".config/kamfw/.envrc" -d "$MODPATH" >&2 || abort "! Failed to extract kamfw runtime files"

export MODDIR="${MODDIR:-$MODPATH}"

# shellcheck disable=SC1090
. "$MODDIR/lib/kamfw/.kamfwrc" || abort "! Failed to source .kamfwrc"
import __runtime__ || abort "! Failed to load kamfw runtime"
import __customize__ || abort "! Failed to load kamfw installer"

(
    # kamfw stores filter patterns through shell evaluation; disable pathname
    # expansion so the wildcard rules remain literal patterns.
    set -f
    install_reset_filters
    install_exclude "META-INF/*" "lib/kamfw/*"
    installer run "$ZIPFILE"
) || abort "! Failed to install module files"
kamfw run install -- "$@" || abort "! kamfw install phase failed"

exit 0
