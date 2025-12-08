#!/bin/bash

. $KAM_HOOKS_ROOT/lib/utils.sh

if [ "$KAM_RELEASE_ENABLED" != "1" ]; then
    echo "Release is disabled, skipping upload"
    exit 0
fi

require_command gh

TMP_CHANGELOG=""
cleanup_tmp() {
    if [ -n "$TMP_CHANGELOG" ] && [ -f "$TMP_CHANGELOG" ]; then
        rm -f "$TMP_CHANGELOG"
        TMP_CHANGELOG=""
    fi
}
trap cleanup_tmp EXIT

get_changelog_path() {
    # Prefer project-level CHANGELOG.md
    if [ -f "${KAM_PROJECT_ROOT}/CHANGELOG.md" ]; then
        echo "${KAM_PROJECT_ROOT}/CHANGELOG.md"
        return 0
    fi

    # Check module root (some modules may keep it there)
    if [ -f "${KAM_MODULE_ROOT}/CHANGELOG.md" ]; then
        echo "${KAM_MODULE_ROOT}/CHANGELOG.md"
        return 0
    fi

    # If a changelog was provided via KAM_MODULE_CHANGELOG, try to use it.
    if [ -n "$KAM_MODULE_CHANGELOG" ]; then
        # If it's a URL, attempt to download it into a temp file
        if echo "$KAM_MODULE_CHANGELOG" | grep -qE '^https?://'; then
            if command -v curl >/dev/null 2>&1; then
                TMP_CHANGELOG=$(mktemp)
                if curl -fsSL "$KAM_MODULE_CHANGELOG" -o "$TMP_CHANGELOG"; then
                    echo "$TMP_CHANGELOG"
                    return 0
                fi
                rm -f "$TMP_CHANGELOG" 2>/dev/null || true
                TMP_CHANGELOG=""
            elif command -v wget >/dev/null 2>&1; then
                TMP_CHANGELOG=$(mktemp)
                if wget -qO "$TMP_CHANGELOG" "$KAM_MODULE_CHANGELOG"; then
                    echo "$TMP_CHANGELOG"
                    return 0
                fi
                rm -f "$TMP_CHANGELOG" 2>/dev/null || true
                TMP_CHANGELOG=""
            fi
        else
            # Treat as local path if it exists
            if [ -f "$KAM_MODULE_CHANGELOG" ]; then
                echo "$KAM_MODULE_CHANGELOG"
                return 0
            fi
        fi
    fi

    # Try a best-effort fetch from the GitHub repository if available
    if [ -n "$GITHUB_REPOSITORY" ]; then
        if command -v curl >/dev/null 2>&1; then
            TMP_CHANGELOG=$(mktemp)
            if curl -fsSL "https://raw.githubusercontent.com/${GITHUB_REPOSITORY}/main/CHANGELOG.md" -o "$TMP_CHANGELOG"; then
                echo "$TMP_CHANGELOG"
                return 0
            fi
            rm -f "$TMP_CHANGELOG" 2>/dev/null || true
            TMP_CHANGELOG=""
        elif command -v wget >/dev/null 2>&1; then
            TMP_CHANGELOG=$(mktemp)
            if wget -qO "$TMP_CHANGELOG" "https://raw.githubusercontent.com/${GITHUB_REPOSITORY}/main/CHANGELOG.md"; then
                echo "$TMP_CHANGELOG"
                return 0
            fi
            rm -f "$TMP_CHANGELOG" 2>/dev/null || true
            TMP_CHANGELOG=""
        fi
    fi

    # Nothing found
    return 1
}

# Extract a single header-section from CHANGELOG.md for a given version.
# Matches headings such as:
#   ## [1.2.3]
#   ## 1.2.3
#   ## v1.2.3
#   ## [v1.2.3]
extract_changelog_section() {
    local file="$1"
    local version="$2"
    if [ ! -f "$file" ]; then
        return 1
    fi

    # Escape dots in version for regex usage
    local ver_escaped
    ver_escaped="${version//./\\.}"

    # Use awk to match the version header and print until next top-level header (## )
    awk -v ver="$ver_escaped" '
    BEGIN {
        # Constructs a regexp that matches common version headings:
        #  ## [1.2.3] or ## v1.2.3 or ## 1.2.3
        regex = "^##[[:space:]]*(\\[?v?" ver "\\]?)"
    }
    $0 ~ regex {
        found = 1
        next
    }
    /^##[[:space:]]/ && found {
        # done for this section
        exit
    }
    found { print }
    ' "$file"
}

# Find changelog path and extract the appropriate section
CHANGELOG_PATH=$(get_changelog_path 2>/dev/null || true)

CHANGELOG_SECTION=""
if [ -n "$CHANGELOG_PATH" ]; then
    CHANGELOG_SECTION=$(extract_changelog_section "$CHANGELOG_PATH" "$KAM_MODULE_VERSION" 2>/dev/null || true)
fi

# If no changelog section for this version, try "Unreleased"
if [ -z "$CHANGELOG_SECTION" ] && [ -n "$CHANGELOG_PATH" ]; then
    CHANGELOG_SECTION=$(extract_changelog_section "$CHANGELOG_PATH" "Unreleased" 2>/dev/null || true)
fi

# If still empty, try to produce a changelog from git commits
if [ -z "$CHANGELOG_SECTION" ] && command -v git >/dev/null 2>&1; then
    log_info "No changelog section found in CHANGELOG.md; falling back to git log"

    # Attempt to detect the previous tag; if it exists, list commits since previous tag
    PREV_TAG=$(git tag --sort=-creatordate | grep -v "^${KAM_MODULE_VERSION}$" | sed -n '1p' 2>/dev/null || true)

    if [ -n "$PREV_TAG" ]; then
        CHANGELOG_SECTION=$(git log --pretty=format:'- %s' "${PREV_TAG}"..HEAD 2>/dev/null || true)
    else
        # No previous tag available — return the last 50 commit messages by default
        CHANGELOG_SECTION=$(git log --pretty=format:'- %s' -n 50 2>/dev/null || true)
    fi
fi

# As a final fallback, include a link to the changelog file
if [ -z "$CHANGELOG_SECTION" ]; then
    CHANGELOG_SECTION="- See [CHANGELOG.md](https://github.com/\${GITHUB_REPOSITORY}/blob/main/CHANGELOG.md) for detailed changes."
fi

# Trim trailing newlines for a cleaner block (optional)
# Note: preserving formatting as-is, but strip leading/trailing blank lines
CHANGELOG_SECTION="$(printf "%s\n" "$CHANGELOG_SECTION" | sed -e :a -e 's/^[[:space:]]*\n//' -e 's/\n[[:space:]]*$//' -e ';ta')"

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
- [${KAM_MODULE_ID}.zip](https://github.com/\${GITHUB_REPOSITORY}/releases/download/${KAM_MODULE_VERSION}/${KAM_MODULE_ID}.zip)

## Installation
1. Download the module ZIP file
2. Install via Magisk/KernelSU/APatch Manager
3. Reboot your device

## Changelog
${CHANGELOG_SECTION}

---
Built with [Kam](https://github.com/MemDeco-WG/Kam)
EOF
)

# Prefer GitHub auto-generated release notes by default.
# Use KAM_RELEASE_GENERATE_NOTES=0 to opt-out and fall back to the handcrafted release notes.
if [ "${KAM_RELEASE_GENERATE_NOTES:-1}" != "0" ]; then
    if gh release create "$KAM_MODULE_VERSION" \
        --title "${KAM_MODULE_NAME} v${KAM_MODULE_VERSION}" \
        --generate-notes \
        "$KAM_DIST_DIR/*"; then
        log_success "Release created using GitHub generated notes."
    else
        log_warn "gh release create with --generate-notes failed; falling back to manual notes."
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
