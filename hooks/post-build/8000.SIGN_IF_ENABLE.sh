#!/bin/bash

. "$KAM_HOOKS_ROOT/lib/utils.sh"

# Sign artifacts in $KAM_DIST_DIR if KAM_SIGN_ENABLE=1
if [ "$KAM_SIGN_ENABLE" != "1" ]; then
	log_info "KAM_SIGN_ENABLE != 1, skipping signing"
	exit 0
fi

require_command kam

DIST=${KAM_DIST_DIR:-$KAM_PROJECT_ROOT/dist}

if [ ! -d "$DIST" ]; then
	log_warn "Dist directory $DIST not found; nothing to sign"
	exit 0
fi

log_info "Signing artifacts in $DIST using 'kam sign --dist'"

# Skip template bundle
if [ -f "$DIST/templates.zip" ]; then
	log_info "Skipping template bundle: $DIST/templates.zip"
fi

# Build base command
CMD=(kam sign --dist "$DIST" --sigstore --timestamp)

# Respect environment overrides
if [ -n "$KAM_SIGN_KEY_PATH" ]; then
	CMD+=(--key-path "$KAM_SIGN_KEY_PATH")
fi
if [ -n "$KAM_SIGN_SECRET" ]; then
	CMD+=(--secret "$KAM_SIGN_SECRET")
fi
if [ "${KAM_SIGN_SIGSTORE:-1}" != "1" ]; then
	# Disable sigstore by not passing --sigstore; our 'kam sign' defaults timestamp true
	CMD=(kam sign --dist "$DIST" --timestamp)
fi
if [ "${KAM_SIGN_ATTESTATION_ONLY:-0}" = "1" ]; then
	CMD+=(--attestation-only)
fi

# Execute sign command once (it will iterate inside kam sign over files)
log_info "Running: ${CMD[*]}"
if "${CMD[@]}"; then
	log_success "Signed artifacts in $DIST"
else
	log_warn "Signing failed for some artifacts in $DIST (continuing)"
fi

exit 0
