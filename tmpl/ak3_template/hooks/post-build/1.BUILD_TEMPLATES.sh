#!/bin/sh
# 2.BUILD_TEMPLATES.sh - compress all files in templates directory into dist/templates.zip

. "$KAM_HOOKS_ROOT/lib/utils.sh"

# Where to output the ZIP (KAM_DIST_DIR preferred, default to KAM_PROJECT_ROOT/dist)
DIST="${KAM_DIST_DIR:-$KAM_PROJECT_ROOT/dist}"
TEMPLATES_DIR="$KAM_PROJECT_ROOT/templates"

# Nothing to do if templates directory doesn't exist
if [ ! -d "$TEMPLATES_DIR" ]; then
  log_info "No templates directory found; skipping templates packaging."
  exit 0
fi

# Ensure zip is available
require_command "zip"

# Ensure output directory exists
mkdir -p "$DIST"

# Remove existing zip to avoid appending to it
rm -f "$DIST/templates.zip"

# Create a minimal archive of the contents of templates/

zip -rj "$DIST/templates.zip" $TEMPLATES_DIR || exit 1

log_success "Templates packaged at $DIST/templates.zip"
exit 0
