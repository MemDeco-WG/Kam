#!/bin/bash
# Example post-build hook script
# This script runs after the build process completes.
# IMPORTANT: Use the KAM_* environment variables injected by Kam
# (for example: $KAM_MODULE_ROOT, $KAM_PROJECT_ROOT, $KAM_DIST_DIR, $KAM_HOOKS_ROOT).
# Do NOT hard-code paths like 'src/<id>'; reference the module source via $KAM_MODULE_ROOT
# and artifacts via $KAM_DIST_DIR to avoid confusion across different module configurations.

. "$KAM_HOOKS_ROOT/lib/utils.sh"

# Add your post-build logic here (e.g., signing the zip, uploading artifacts)
