#!/bin/sh
# Sync kam.toml [prop] section to module.prop
# This hook generates module.prop from kam.toml before the build process starts.

# Source common utilities
if [ -f "$KAM_HOOKS_ROOT/lib/utils.sh" ]; then
    . "$KAM_HOOKS_ROOT/lib/utils.sh"
else
    echo "Warning: utils.sh not found at $KAM_HOOKS_ROOT/lib/utils.sh"
    log_info() { echo "[INFO] $1"; }
    log_warn() { echo "[WARN] $1"; }
    log_error() { echo "[ERROR] $1"; }
    log_success() { echo "[SUCCESS] $1"; }
fi

log_info "Syncing kam.toml [prop] section to module.prop..."

# Check if required KAM environment variables are set
if [ -z "$KAM_MODULE_ID" ] || [ -z "$KAM_MODULE_VERSION" ] || [ -z "$KAM_MODULE_VERSION_CODE" ]; then
    log_error "Required KAM_MODULE_* environment variables are not set"
    exit 1
fi

# Determine module.prop location
# For standard Magisk/KernelSU modules, it should be in src/<module_id>/module.prop
MODULE_PROP_PATH="${KAM_PROJECT_ROOT}/src/${KAM_MODULE_ID}/module.prop"

# Check if the directory exists
MODULE_DIR=$(dirname "$MODULE_PROP_PATH")
if [ ! -d "$MODULE_DIR" ]; then
    log_warn "Module directory does not exist: $MODULE_DIR"
    log_info "Attempting to create directory..."
    mkdir -p "$MODULE_DIR" || {
        log_error "Failed to create directory: $MODULE_DIR"
        exit 1
    }
fi

# Generate module.prop content from KAM environment variables
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

log_info "kam.toml → module.prop sync completed"
