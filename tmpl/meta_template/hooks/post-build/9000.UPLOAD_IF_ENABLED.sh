#!/bin/bash

. "$KAM_HOOKS_ROOT/lib/utils.sh"

# Exit if release is disabled
if [ "$KAM_RELEASE_ENABLED" != "1" ]; then
    log_warn "Release is disabled, skipping upload"
    exit 0
fi

require_command gh

TAG=${KAM_RELEASE_TAG:-$KAM_MODULE_VERSION}

# Prepare release notes
TMP_CHANGELOG=$(mktemp)
cleanup_tmp() {
    if [ -n "$TMP_CHANGELOG" ] && [ -f "$TMP_CHANGELOG" ]; then
        rm -f "$TMP_CHANGELOG"
        TMP_CHANGELOG=""
    fi
}
trap cleanup_tmp EXIT

# Attempt to extract changelog section for this version from CHANGELOG.md
CHANGELOG_SECTION=""
if [ -f "$KAM_PROJECT_ROOT/CHANGELOG.md" ]; then
    CHANGELOG_SECTION=$(awk -v ver="${KAM_MODULE_VERSION}" 'BEGIN{found=0} $0 ~ ver {found=1; next} found && /^#+[ ]/ {exit} found{print}' "$KAM_PROJECT_ROOT/CHANGELOG.md" || true)
fi
if [ -z "$CHANGELOG_SECTION" ] && command -v git >/dev/null 2>&1; then
    PREV_TAG=$(git tag --sort=-creatordate | grep -v "^${KAM_MODULE_VERSION}$" | sed -n '1p' 2>/dev/null || true)
    if [ -n "$PREV_TAG" ]; then
        CHANGELOG_SECTION=$(git log --pretty=format:'- %s' "${PREV_TAG}"..HEAD 2>/dev/null || true)
    else
        CHANGELOG_SECTION=$(git log --pretty=format:'- %s' -n 50 2>/dev/null || true)
    fi
fi
if [ -z "$CHANGELOG_SECTION" ]; then
    CHANGELOG_SECTION="- See CHANGELOG.md for detailed changes."
fi

RELEASE_NOTES=$(cat <<EOF
# ${KAM_MODULE_NAME} v${KAM_MODULE_VERSION}

## Module Information
- **Version**: ${KAM_MODULE_VERSION}
- **Version Code**: ${KAM_MODULE_VERSION_CODE}
- **Module ID**: ${KAM_MODULE_ID}
- **Author**: ${KAM_MODULE_AUTHOR}

## Description
${KAM_MODULE_DESCRIPTION}

## Download
- [${KAM_MODULE_ID}.zip](https://github.com/${KAM_GITHUB_REPO}/releases/download/${KAM_MODULE_VERSION}/${KAM_MODULE_ID}.zip)

## Changelog
${CHANGELOG_SECTION}

---
Built with [Kam](https://github.com/MemDeco-WG/Kam)
EOF
)
printf "%s\n" "$RELEASE_NOTES" > "$TMP_CHANGELOG"

# Create release if it does not exist, otherwise edit notes
if ! gh release view "$TAG" >/dev/null 2>&1; then
    PRE_FLAG=""
    if [ "${KAM_PRE_RELEASE:-0}" = "1" ]; then
        PRE_FLAG="--prerelease"
    fi
    gh release create "$TAG" \
        --title "$KAM_MODULE_ID" \
        --notes-file "$TMP_CHANGELOG" \
        $PRE_FLAG || log_warn "Failed to create release $TAG"
else
    gh release edit "$TAG" --title "$KAM_MODULE_ID" --notes-file "$TMP_CHANGELOG" || log_warn "Failed to edit release $TAG"
fi

log_info "Uploading attestation JSON assets from $KAM_DIST_DIR to release $TAG"

for f in "$KAM_DIST_DIR"/*.attestation.json "$KAM_DIST_DIR"/*.sigstore.json; do
    [ -f "$f" ] || continue
    log_info "Uploading asset: $f"
    gh release upload "$TAG" "$f" --clobber || log_warn "Failed to upload $f"
done

log_success "Upload complete"

exit 0
