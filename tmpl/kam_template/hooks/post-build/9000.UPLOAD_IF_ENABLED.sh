#!/bin/bash
. "$KAM_HOOKS_ROOT/lib/utils.sh"

# Exit if disabled
[ "$KAM_RELEASE_ENABLED" != "1" ] && echo "Release disabled" && exit 0

require_command gh

PRE_RELEASE_FLAG=$([ "$KAM_PRE_RELEASE" = "1" ] && echo "--prerelease")

# Immutable
if [ "$KAM_IMMUTABLE_RELEASE" = "1" ] && gh release view "$KAM_MODULE_VERSION" >/dev/null 2>&1; then
    log_info "Immutable release exists; skip."
    exit 0
fi

# Collect assets
ASSET_ARGS=()
for a in "$KAM_DIST_DIR"/*; do
    [ -f "$a" ] && ASSET_ARGS+=("$a")

    if [ "$KAM_SIGN_ENABLE" = "1" ]; then
        for ext in sig tsr sigstore.json attestation.json; do
            [ -f "$a.$ext" ] && ASSET_ARGS+=("$a.$ext")
        done
    fi
done

# Create or update release (gh auto-generate notes)
if gh release view "$KAM_MODULE_VERSION" >/dev/null 2>&1; then
    log_info "Release exists → updating"
    gh release edit "$KAM_MODULE_VERSION" \
        --title "${KAM_MODULE_NAME} v${KAM_MODULE_VERSION}" \
        $PRE_RELEASE_FLAG \
        --generate-notes
else
    log_info "Creating release"
    gh release create "$KAM_MODULE_VERSION" \
        --title "${KAM_MODULE_NAME} v${KAM_MODULE_VERSION}" \
        $PRE_RELEASE_FLAG \
        --generate-notes
fi

# Upload assets
for f in "${ASSET_ARGS[@]}"; do
    log_info "Uploading $f"
    gh release upload "$KAM_MODULE_VERSION" "$f" --clobber
done

log_info "Upload complete"
