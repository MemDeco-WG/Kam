#!/bin/bash

. $KAM_HOOKS_ROOT/lib/utils.sh

# Exit if release is disabled
if [ "$KAM_RELEASE_ENABLED" != "1" ]; then
    echo "Release is disabled, skipping upload"
    exit 0
fi

# Ensure the gh command is available
require_command gh

# Temporary changelog file for download
TMP_CHANGELOG=""

# Determine the GitHub repository using the gh CLI (primary method).
# `require_command gh` has already ensured gh is present.
GITHUB_REPO=$(gh repo view --json nameWithOwner --jq '.nameWithOwner' 2>/dev/null || true)
cleanup_tmp() {
    if [ -n "$TMP_CHANGELOG" ] && [ -f "$TMP_CHANGELOG" ]; then
        rm -f "$TMP_CHANGELOG"
        TMP_CHANGELOG=""
    fi
}
trap cleanup_tmp EXIT

# Function to fetch or define the changelog path
get_changelog_path() {
    if [ -f "${KAM_PROJECT_ROOT}/CHANGELOG.md" ]; then
        echo "${KAM_PROJECT_ROOT}/CHANGELOG.md"
        return 0
    fi

    if [ -f "${KAM_MODULE_ROOT}/CHANGELOG.md" ]; then
        echo "${KAM_MODULE_ROOT}/CHANGELOG.md"
        return 0
    fi

    if [ -n "$KAM_MODULE_CHANGELOG" ]; then
        if echo "$KAM_MODULE_CHANGELOG" | grep -qE '^https?://'; then
            # Attempt to download changelog if URL is provided
            if command -v curl >/dev/null 2>&1; then
                TMP_CHANGELOG=$(mktemp)
                if curl -fsSL "$KAM_MODULE_CHANGELOG" -o "$TMP_CHANGELOG"; then
                    echo "$TMP_CHANGELOG"
                    return 0
                fi
                rm -f "$TMP_CHANGELOG" 2>/dev/null || true
            elif command -v wget >/dev/null 2>&1; then
                TMP_CHANGELOG=$(mktemp)
                if wget -qO "$TMP_CHANGELOG" "$KAM_MODULE_CHANGELOG"; then
                    echo "$TMP_CHANGELOG"
                    return 0
                fi
                rm -f "$TMP_CHANGELOG" 2>/dev/null || true
            fi
        else
            if [ -f "$KAM_MODULE_CHANGELOG" ]; then
                echo "$KAM_MODULE_CHANGELOG"
                return 0
            fi
        fi
    fi

    # Attempt to fetch from GitHub
    # Prefer the gh CLI for authenticated/raw retrieval; fall back to curl/wget if needed.
    if [ -n "$GITHUB_REPO" ]; then
        if command -v gh >/dev/null 2>&1; then
            TMP_CHANGELOG=$(mktemp)
            if gh api -H "Accept: application/vnd.github.v3.raw" "/repos/${GITHUB_REPO}/contents/CHANGELOG.md" > "$TMP_CHANGELOG" 2>/dev/null; then
                echo "$TMP_CHANGELOG"
                return 0
            fi
            rm -f "$TMP_CHANGELOG" 2>/dev/null || true
            TMP_CHANGELOG=""
        fi

        # Try curl/wget directly against raw.githubusercontent.com if gh api is not available
        if command -v curl >/dev/null 2>&1; then
            TMP_CHANGELOG=$(mktemp)
            if curl -fsSL "https://raw.githubusercontent.com/${GITHUB_REPO}/main/CHANGELOG.md" -o "$TMP_CHANGELOG"; then
                echo "$TMP_CHANGELOG"
                return 0
            fi
            rm -f "$TMP_CHANGELOG" 2>/dev/null || true
            TMP_CHANGELOG=""
        elif command -v wget >/dev/null 2>&1; then
            TMP_CHANGELOG=$(mktemp)
            if wget -qO "$TMP_CHANGELOG" "https://raw.githubusercontent.com/${GITHUB_REPO}/main/CHANGELOG.md"; then
                echo "$TMP_CHANGELOG"
                return 0
            fi
            rm -f "$TMP_CHANGELOG" 2>/dev/null || true
            TMP_CHANGELOG=""
        fi
    fi

    return 1
}

# Extract the changelog section for a given version
extract_changelog_section() {
    local file="$1"
    local version="$2"
    if [ ! -f "$file" ]; then
        return 1
    fi

    local ver_escaped
    ver_escaped="${version//./\\.}"

    # Match headings like ## [1.2.3], ## v1.2.3
    awk -v ver="$ver_escaped" '
    BEGIN {
        regex = "^##[[:space:]]*(\\[?v?" ver "\\]?)"
    }
    $0 ~ regex { found = 1; next }
    /^##[[:space:]]/ && found { exit }
    found { print }
    ' "$file"
}

# Get changelog path and section
CHANGELOG_PATH=$(get_changelog_path 2>/dev/null || true)

CHANGELOG_SECTION=""
if [ -n "$CHANGELOG_PATH" ]; then
    CHANGELOG_SECTION=$(extract_changelog_section "$CHANGELOG_PATH" "$KAM_MODULE_VERSION" 2>/dev/null || true)
fi

# Fallback to "Unreleased" section if not found
if [ -z "$CHANGELOG_SECTION" ] && [ -n "$CHANGELOG_PATH" ]; then
    CHANGELOG_SECTION=$(extract_changelog_section "$CHANGELOG_PATH" "Unreleased" 2>/dev/null || true)
fi

# Fallback to git log if no changelog section
if [ -z "$CHANGELOG_SECTION" ] && command -v git >/dev/null 2>&1; then
    log_info "No changelog section found, falling back to git log"
    PREV_TAG=$(git tag --sort=-creatordate | grep -v "^${KAM_MODULE_VERSION}$" | sed -n '1p' 2>/dev/null || true)

    if [ -n "$PREV_TAG" ]; then
        CHANGELOG_SECTION=$(git log --pretty=format:'- %s' "${PREV_TAG}"..HEAD 2>/dev/null || true)
    else
        CHANGELOG_SECTION=$(git log --pretty=format:'- %s' -n 50 2>/dev/null || true)
    fi
fi

# Fallback message if no changelog
if [ -z "$CHANGELOG_SECTION" ]; then
    if [ -n "$GITHUB_REPO" ]; then
        CHANGELOG_SECTION="- See [CHANGELOG.md](https://github.com/${GITHUB_REPO}/blob/main/CHANGELOG.md) for detailed changes."
    else
        CHANGELOG_SECTION="- See CHANGELOG.md for detailed changes."
    fi
fi

# Format changelog section (remove leading/trailing empty lines)
CHANGELOG_SECTION="$(printf "%s\n" "$CHANGELOG_SECTION" | sed -e :a -e 's/^[[:space:]]*\n//' -e 's/\n[[:space:]]*$//' -e ';ta')"

# Release notes template
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
- [${KAM_MODULE_ID}.zip](https://github.com/${GITHUB_REPO}/releases/download/${KAM_MODULE_VERSION}/${KAM_MODULE_ID}.zip)


## Changelog
${CHANGELOG_SECTION}

---
Built with [Kam](https://github.com/MemDeco-WG/Kam)
EOF
)

# Release creation using GitHub CLI
if [ "${KAM_RELEASE_GENERATE_NOTES:-1}" != "0" ]; then
    if gh release create "$KAM_MODULE_VERSION" \
        --title "${KAM_MODULE_NAME} v${KAM_MODULE_VERSION}" \
        --generate-notes \
        "$KAM_DIST_DIR/*"; then
        log_success "Release created with auto-generated notes."
    else
        log_warn "Failed to generate notes, falling back to manual release notes."
        gh release create "$KAM_MODULE_VERSION" \
            --title "${KAM_MODULE_NAME} v${KAM_MODULE_VERSION}" \
            --notes "$RELEASE_NOTES" \
            "$KAM_DIST_DIR/*"
    fi
else
    gh release create "$KAM_MODULE_VERSION" \
        --title "${KAM_MODULE_NAME} v${KAM_MODULE_VERSION}" \
        --notes "$RELEASE_NOTES" \
        "$KAM_DIST_DIR/*"
fi

echo "Upload complete"
