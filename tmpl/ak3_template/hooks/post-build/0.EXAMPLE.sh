#!/bin/sh
# Example post-build hook script
# This script runs after the build process completes.

# Source common utilities
if [ -f "$KAM_HOOKS_ROOT/lib/utils.sh" ]; then
    . "$KAM_HOOKS_ROOT/lib/utils.sh"
else
    echo "Warning: utils.sh not found at $KAM_HOOKS_ROOT/lib/utils.sh"
    # Define fallback log_info if utils.sh is missing
    log_info() { echo "[INFO] $1"; }
fi

log_info "Running tmpl post-build hook..."
log_info "Module built successfully."

# Add your post-build logic here (e.g., signing the zip, uploading artifacts)
