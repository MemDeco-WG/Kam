#!/bin/bash
# Sync kam.toml to module.prop and update.json
# This hook generates:
# - module.prop in module directory ($KAM_MODULE_ROOT/module.prop)
# - update.json in project root ($KAM_PROJECT_ROOT/update.json)

. "$KAM_HOOKS_ROOT/lib/utils.sh"

log_info "Syncing kam.toml to module.prop and update.json..."

# Check if required KAM environment variables are set
if [ -z "$KAM_MODULE_ID" ] || [ -z "$KAM_MODULE_VERSION" ] || [ -z "$KAM_MODULE_VERSION_CODE" ]; then
    log_error "Required KAM_MODULE_* environment variables are not set"
    exit 1
fi

# Skip template modules (modules with id ending in _template)
case "$KAM_MODULE_ID" in
    *_template)
        log_info "Skipping template module: $KAM_MODULE_ID"
        exit 0
        ;;
esac

# Determine file paths
# module.prop goes to module directory
MODULE_PROP_PATH="${KAM_MODULE_ROOT}/module.prop"
# update.json goes to project root directory
UPDATE_JSON_PATH="${KAM_PROJECT_ROOT}/update.json"

# Check if the module root directory exists
if [ ! -d "$KAM_MODULE_ROOT" ]; then
    log_warn "Module directory does not exist: $KAM_MODULE_ROOT"
    log_info "Attempting to create directory..."
    mkdir -p "$KAM_MODULE_ROOT" || {
        log_error "Failed to create directory: $KAM_MODULE_ROOT"
        exit 1
    }
fi

###########################################
# Sync module.prop
###########################################
log_info "Generating module.prop at: $MODULE_PROP_PATH"

cat > "$MODULE_PROP_PATH" << EOF
id=${KAM_MODULE_ID}
name=${KAM_MODULE_NAME}
version=${KAM_MODULE_VERSION}
versionCode=${KAM_MODULE_VERSION_CODE}
author=${KAM_MODULE_AUTHOR}
description=${KAM_MODULE_DESCRIPTION}
EOF

# Add updateJson if set (optional field)
if [ -n "$KAM_MODULE_UPDATE_JSON" ]; then
    echo "updateJson=${KAM_MODULE_UPDATE_JSON}" >> "$MODULE_PROP_PATH"
fi

# Verify the file was created successfully
if [ -f "$MODULE_PROP_PATH" ]; then
    log_success "module.prop synced successfully"

    # Show content if debug mode is enabled
    if [ "${KAM_DEBUG:-}" = "1" ]; then
        log_info "module.prop content:"
        while IFS= read -r line; do
            printf "  %s\n" "$line"
        done < "$MODULE_PROP_PATH"
    fi
else
    log_error "Failed to create module.prop at: $MODULE_PROP_PATH"
    exit 1
fi

###########################################
# Sync update.json
###########################################
log_info "Generating update.json at: $UPDATE_JSON_PATH"

# Try to read kam.toml to extract repository and changelog info
KAM_TOML_PATH="${KAM_PROJECT_ROOT}/kam.toml"
REPOSITORY_URL=""
CHANGELOG_URL=""

if [ -f "$KAM_TOML_PATH" ]; then
    # Try to extract repository URL from kam.toml using grep and sed
    # Look for homepage, repository, or other URL fields in [mmrl.repo] section
    REPOSITORY_URL=$(grep -A 20 '^\[mmrl\.repo\]' "$KAM_TOML_PATH" 2>/dev/null | \
                     grep -E '^\s*(repository|homepage)\s*=' | \
                     head -n 1 | \
                     sed 's/^[^=]*=\s*"\([^"]*\)".*/\1/' | \
                     sed 's/{%[^%]*%}//g' | \
                     sed 's/{{[^}]*}}//g' | \
                     tr -d ' ')

    # Try to extract changelog URL from kam.toml
    CHANGELOG_URL=$(grep -A 20 '^\[mmrl\.repo\]' "$KAM_TOML_PATH" 2>/dev/null | \
                   grep -E '^\s*changelog\s*=' | \
                   head -n 1 | \
                   sed 's/^[^=]*=\s*"\([^"]*\)".*/\1/' | \
                   sed 's/{%[^%]*%}//g' | \
                   sed 's/{{[^}]*}}//g' | \
                   tr -d ' ')
fi

# Fallback to environment variables if available
if [ -n "$KAM_MODULE_REPOSITORY" ]; then
    REPOSITORY_URL="$KAM_MODULE_REPOSITORY"
fi

if [ -n "$KAM_MODULE_CHANGELOG" ]; then
    CHANGELOG_URL="$KAM_MODULE_CHANGELOG"
fi

# Determine zipUrl
if [ -n "$REPOSITORY_URL" ] && [ "$REPOSITORY_URL" != "" ]; then
    # If repository URL is from GitHub, construct the release URL
    case "$REPOSITORY_URL" in
        *github.com*)
            ZIP_URL="${REPOSITORY_URL}/releases/latest/download/${KAM_MODULE_ID}.zip"
            ;;
        *)
            # For other platforms, use a generic pattern
            ZIP_URL="${REPOSITORY_URL}/releases/latest/download/${KAM_MODULE_ID}.zip"
            ;;
    esac
else
    # Default fallback
    ZIP_URL="https://github.com/user/repo/releases/latest/download/${KAM_MODULE_ID}.zip"
fi

# Determine changelog URL
if [ -n "$CHANGELOG_URL" ] && [ "$CHANGELOG_URL" != "" ]; then
    # Use the changelog URL from kam.toml
    FINAL_CHANGELOG_URL="$CHANGELOG_URL"
elif [ -n "$REPOSITORY_URL" ] && [ "$REPOSITORY_URL" != "" ]; then
    # Try to construct changelog URL from repository
    case "$REPOSITORY_URL" in
        *github.com*)
            # Convert https://github.com/user/repo to raw URL
            FINAL_CHANGELOG_URL="${REPOSITORY_URL}/raw/main/CHANGELOG.md"
            ;;
        *)
            # For other platforms, try a similar pattern
            FINAL_CHANGELOG_URL="${REPOSITORY_URL}/CHANGELOG.md"
            ;;
    esac
else
    # Default fallback
    FINAL_CHANGELOG_URL="https://raw.githubusercontent.com/user/repo/main/CHANGELOG.md"
fi

# Generate update.json with proper JSON formatting
cat > "$UPDATE_JSON_PATH" << EOF
{
  "version": "${KAM_MODULE_VERSION}",
  "versionCode": ${KAM_MODULE_VERSION_CODE},
  "zipUrl": "${ZIP_URL}",
  "changelog": "${FINAL_CHANGELOG_URL}"
}
EOF

# Verify the file was created successfully
if [ -f "$UPDATE_JSON_PATH" ]; then
    log_success "update.json synced successfully"

    # Show content if debug mode is enabled
    if [ "${KAM_DEBUG:-}" = "1" ]; then
        log_info "update.json content:"
        while IFS= read -r line; do
            printf "  %s\n" "$line"
        done < "$UPDATE_JSON_PATH"
    fi
else
    log_error "Failed to create update.json at: $UPDATE_JSON_PATH"
    exit 1
fi

log_success "kam.toml → module.prop & update.json sync completed"
