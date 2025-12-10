#!/bin/bash

. "$KAM_HOOKS_ROOT/lib/utils.sh"

# Sign artifacts in $KAM_DIST_DIR if KAM_SIGN_ENABLED=1
if [ "$KAM_SIGN_ENABLED" != "1" ]; then
	log_info "KAM_SIGN_ENABLED != 1, skipping signing"
	exit 0
fi

require_command kam

DIST=${KAM_DIST_DIR:-$KAM_PROJECT_ROOT/dist}

if [ ! -d "$DIST" ]; then
	log_warn "Dist directory $DIST not found; nothing to sign"
	exit 0
fi

log_info "Signing artifacts in $DIST using 'kam sign'"

kam sign $DIST/$KAM_MODULE_ID-*.zip --out $DIST --sigstore -t


