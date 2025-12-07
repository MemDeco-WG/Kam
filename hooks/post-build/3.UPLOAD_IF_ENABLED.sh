#!/bin/bash

. $KAM_HOOKS_ROOT/lib/utils.sh

if [ "$KAM_RELEASE_ENABLED" != "1" ]; then
    echo "Release is disabled, skipping upload"
    exit 0
fi

if ! require_command gh; then
    echo "gh command not found"
    exit 1
fi

gh release create "$KAM_MODULE_VERSION" --title "$KAM_MODULE_VERSION" --notes "$KAM_MODULE_VERSION" "$KAM_DIST_DIR/$KAM_MODULE_ID.zip"

echo "Upload complete"
