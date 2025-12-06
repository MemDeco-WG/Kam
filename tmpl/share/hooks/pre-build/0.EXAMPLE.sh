#!/bin/sh
# Example pre-build hook script
# This script runs before the build process starts.

# Source common utilities
if [ -f "$KAM_HOOKS_ROOT/lib/utils.sh" ]; then
    . "$KAM_HOOKS_ROOT/lib/utils.sh"
else
    echo "Warning: utils.sh not found at $KAM_HOOKS_ROOT/lib/utils.sh"
    # Define fallback log_info if utils.sh is missing
    log_info() { echo "[INFO] $1"; }
fi

log_info "Running tmpl pre-build hook..."
log_info "Building module: $KAM_MODULE_ID v$KAM_MODULE_VERSION"

# Add your pre-build logic here (e.g., downloading assets, checking environment)
