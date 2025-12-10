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

log_info "Signing artifacts in $DIST"

# Iterate artifacts and sign if they are files (skip directories)
for f in "$DIST"/*; do
	if [ -f "$f" ]; then
		# Determine file extension and skip common signature files
		case "${f##*.}" in
			sig|tsr|json)
				log_info "Skipping $f (signature/metadata file)"
				continue
				;;
		esac

		# Generate sign command; default: sigstore + timestamp
		CMD=(kam sign "$f" --sigstore --timestamp)

		# Allow disabling sigstore via env variable
		if [ "${KAM_SIGN_SIGSTORE:-1}" != "1" ]; then
			CMD=(kam sign "$f" --timestamp)
		fi

		log_info "Signing $f"
		if "${CMD[@]}"; then
			log_success "Signed $f"
		else
			log_warn "Signing failed for $f (continuing)"
		fi
	fi
done

exit 0
